use itertools::Itertools;
use p3_air::{Air, TwoRowMatrixView};
use p3_challenger::{CanObserve, FieldChallenger};
use p3_commit::{Pcs, UnivariatePcs, UnivariatePcsWithLde};
use p3_field::{
    cyclic_subgroup_coset_known_order, AbstractExtensionField, AbstractField, Field, PackedField,
    TwoAdicField,
};
use p3_matrix::dense::RowMajorMatrix;
use p3_matrix::{MatrixGet, MatrixRows};
use p3_maybe_rayon::prelude::{IntoParallelIterator, ParIterExt};
use p3_uni_stark::{ProverConstraintFolder, StarkConfig, ZerofierOnCoset};
use p3_util::log2_strict_usize;

use crate::generation::bitshift::generate_bitshift_trace;
use crate::generation::xor::generate_dummy_xor_trace;

const XOR_TRACE_LEN_LOG: u32 = 10;
const NUM_STARKS: usize = 2;

fn quotient_values<SC, A, PreprocessedTraceLde, MainTraceLde, PermTraceLde>(
    config: &SC,
    air: &A,
    log_degree: usize,
    log_quotient_degree: usize,
    preprocessed_trace_lde: Option<PreprocessedTraceLde>,
    main_trace_lde: MainTraceLde,
    perm_trace_lde: PermTraceLde,
    perm_challenges: &[SC::Challenge],
    alpha: SC::Challenge,
) -> Vec<SC::Challenge>
where
    SC: StarkConfig,
    A: for<'a> Air<ProverConstraintFolder<'a, SC>>,
    PreprocessedTraceLde: MatrixRows<SC::Val> + MatrixGet<SC::Val> + Sync,
    MainTraceLde: MatrixRows<SC::Val> + MatrixGet<SC::Val> + Sync,
    PermTraceLde: MatrixRows<SC::Val> + MatrixGet<SC::Val> + Sync, {
    let degree = 1 << log_degree;
    let log_quotient_size = log_degree + log_quotient_degree;
    let quotient_size = 1 << log_quotient_size;
    let g_subgroup = SC::Val::two_adic_generator(log_degree);
    let g_extended = SC::Val::two_adic_generator(log_quotient_size);
    let subgroup_last = g_subgroup.inverse();
    let coset_shift = config.pcs().coset_shift();
    let next_step = 1 << log_quotient_degree;

    let mut coset: Vec<_> =
        cyclic_subgroup_coset_known_order(g_extended, coset_shift, quotient_size).collect();

    let zerofier_on_coset = ZerofierOnCoset::new(log_degree, log_quotient_degree, coset_shift);

    // Evaluations of L_first(x) = Z_H(x) / (x - 1) on our coset s H.
    let mut lagrange_first_evals = zerofier_on_coset.lagrange_basis_unnormalized(0);
    let mut lagrange_last_evals = zerofier_on_coset.lagrange_basis_unnormalized(degree - 1);

    // We have a few vectors of length `quotient_size`, and we're going to take
    // slices therein of length `WIDTH`. In the edge case where `quotient_size <
    // WIDTH`, we need to pad those vectors in order for the slices to exist.
    // The entries beyond quotient_size will be ignored, so we can
    // just use default values.
    for _ in quotient_size..SC::PackedVal::WIDTH {
        coset.push(SC::Val::default());
        lagrange_first_evals.push(SC::Val::default());
        lagrange_last_evals.push(SC::Val::default());
    }

    (0..quotient_size)
        .into_par_iter()
        .step_by(SC::PackedVal::WIDTH)
        .flat_map_iter(|i_local_start| {
            let wrap = |i| i % quotient_size;
            let i_next_start = wrap(i_local_start + next_step);
            let i_range = i_local_start..i_local_start + SC::PackedVal::WIDTH;

            let x = *SC::PackedVal::from_slice(&coset[i_range.clone()]);
            let is_transition = x - subgroup_last;
            let is_first_row = *SC::PackedVal::from_slice(&lagrange_first_evals[i_range.clone()]);
            let is_last_row = *SC::PackedVal::from_slice(&lagrange_last_evals[i_range]);

            let (preprocessed_local, preprocessed_next): (Vec<_>, Vec<_>) =
                match &preprocessed_trace_lde {
                    Some(lde) => {
                        let local = (0..lde.width())
                            .map(|col| {
                                SC::PackedVal::from_fn(|offset| {
                                    let row = wrap(i_local_start + offset);
                                    lde.get(row, col)
                                })
                            })
                            .collect();
                        let next = (0..lde.width())
                            .map(|col| {
                                SC::PackedVal::from_fn(|offset| {
                                    let row = wrap(i_next_start + offset);
                                    lde.get(row, col)
                                })
                            })
                            .collect();
                        (local, next)
                    }
                    None => (vec![], vec![]),
                };

            let main_local: Vec<_> = (0..main_trace_lde.width())
                .map(|col| {
                    SC::PackedVal::from_fn(|offset| {
                        let row = wrap(i_local_start + offset);
                        main_trace_lde.get(row, col)
                    })
                })
                .collect();
            let main_next: Vec<_> = (0..main_trace_lde.width())
                .map(|col| {
                    SC::PackedVal::from_fn(|offset| {
                        let row = wrap(i_next_start + offset);
                        main_trace_lde.get(row, col)
                    })
                })
                .collect();

            let ext_degree = <SC::Challenge as AbstractExtensionField<SC::Val>>::D;
            debug_assert_eq!(perm_trace_lde.width() % ext_degree, 0);
            let perm_width_ext = perm_trace_lde.width() / ext_degree;

            let perm_local: Vec<_> = (0..perm_width_ext)
                .map(|ext_col| {
                    SC::PackedChallenge::from_base_fn(|coeff_idx| {
                        SC::PackedVal::from_fn(|offset| {
                            let row = wrap(i_local_start + offset);
                            perm_trace_lde.get(row, ext_col * ext_degree + coeff_idx)
                        })
                    })
                })
                .collect();
            let perm_next: Vec<_> = (0..perm_width_ext)
                .map(|ext_col| {
                    SC::PackedChallenge::from_base_fn(|coeff_idx| {
                        SC::PackedVal::from_fn(|offset| {
                            let row = wrap(i_next_start + offset);
                            perm_trace_lde.get(row, ext_col * ext_degree + coeff_idx)
                        })
                    })
                })
                .collect();

            let accumulator = SC::PackedChallenge::zero();
            let mut folder = ProverConstraintFolder {
                main: TwoRowMatrixView {
                    local: &main_local,
                    next: &main_next,
                },
                is_first_row,
                is_last_row,
                is_transition,
                alpha,
                accumulator,
            };
            air.eval(&mut folder);

            // quotient(x) = constraints(x) / Z_H(x)
            let zerofier_inv: SC::PackedVal = zerofier_on_coset.eval_inverse_packed(i_local_start);
            let quotient = folder.accumulator * zerofier_inv;

            // "Transpose" D packed base coefficients into WIDTH scalar extension
            // coefficients.
            let limit = SC::PackedVal::WIDTH.min(quotient_size);
            (0..limit).map(move |idx_in_packing| {
                let quotient_value = (0..<SC::Challenge as AbstractExtensionField<SC::Val>>::D)
                    .map(|coeff_idx| quotient.as_base_slice()[coeff_idx].as_slice()[idx_in_packing])
                    .collect_vec();
                SC::Challenge::from_base_slice(&quotient_value)
            })
        })
        .collect()
}

/// Note that this is an incomplete prover. Mainly intended for experiment.
/// # Panics
/// This function will panic if the number of traces is not equal to the number
/// of Starks.
pub fn prove<SC: StarkConfig>(config: &SC, mut challenger: SC::Challenger) {
    // collect traces of each stark as Matrices
    let traces = [
        generate_bitshift_trace(),
        generate_dummy_xor_trace(XOR_TRACE_LEN_LOG),
    ];

    // height of each trace matrix
    let degrees: [usize; NUM_STARKS] = traces
        .iter()
        .map(p3_matrix::Matrix::height)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let log_degrees = degrees.map(log2_strict_usize);
    let g_subgroups = log_degrees.map(SC::Val::two_adic_generator);

    // commit to traces
    let (commit, data) = config.pcs().commit_batches(traces.to_vec());
    challenger.observe(commit.clone());

    let mut preprocessed_trace_ldes = config.pcs().get_ldes(&data);

    let zeta: SC::Challenge = challenger.sample_ext_element();
    let zeta_and_next: [Vec<SC::Challenge>; 2] =
        core::array::from_fn(|i| vec![zeta, zeta * g_subgroups[i]]);
    challenger.observe(commit.clone());

    traces.map(|trace| {
        let preprocessed_trace_lde = trace.map(|trace| preprocessed_trace_ldes.remove(0));
    });

    // let mut quotients: Vec<RowMajorMatrix<SC::Val>> = vec![];
    // let (quotient_commit, quotient_data) = tracing::info_span!("commit to
    // quotient chunks")     .in_scope(||
    // pcs.commit_batches(quotients.to_vec()));
    let prover_data_and_points = [(&data, zeta_and_next.as_slice())];

    // generate openings proof
    let (_openings, _opening_proof) = config
        .pcs()
        .open_multi_batches(&prover_data_and_points, &mut challenger);
}

#[cfg(test)]
mod tests {

    use super::prove;
    use crate::config::{DefaultConfig, Mozak3StarkConfig};

    #[test]
    fn test_prove() {
        let (config, challenger) = DefaultConfig::make_config();
        prove(&config, challenger);
    }
}
