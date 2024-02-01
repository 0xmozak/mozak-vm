use itertools::Itertools;
use p3_air::{Air, TwoRowMatrixView};
use p3_commit::UnivariatePcsWithLde;
use p3_field::{
    cyclic_subgroup_coset_known_order, AbstractExtensionField, AbstractField, Field, PackedField,
    TwoAdicField,
};
use p3_matrix::MatrixGet;
use p3_maybe_rayon::prelude::{IntoParallelIterator, ParIterExt};
use p3_uni_stark::{ProverConstraintFolder, StarkConfig, ZerofierOnCoset};

#[allow(clippy::module_name_repetitions)]
pub fn quotient_values<SC, A, Mat>(
    config: &SC,
    air: &A,
    degree_bits: usize,
    quotient_degree_bits: usize,
    trace_lde: &Mat,
    alpha: SC::Challenge,
) -> Vec<SC::Challenge>
where
    SC: StarkConfig,
    A: for<'a> Air<ProverConstraintFolder<'a, SC>>,
    Mat: MatrixGet<SC::Val> + Sync, {
    let degree = 1 << degree_bits;
    let quotient_size_bits = degree_bits + quotient_degree_bits;
    let quotient_size = 1 << quotient_size_bits;
    let g_subgroup = SC::Val::two_adic_generator(degree_bits);
    let g_extended = SC::Val::two_adic_generator(quotient_size_bits);
    let subgroup_last = g_subgroup.inverse();
    let coset_shift = config.pcs().coset_shift();
    let next_step = 1 << quotient_degree_bits;

    let mut coset: Vec<_> =
        cyclic_subgroup_coset_known_order(g_extended, coset_shift, quotient_size).collect();

    let zerofier_on_coset = ZerofierOnCoset::new(degree_bits, quotient_degree_bits, coset_shift);

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

            let local: Vec<_> = (0..trace_lde.width())
                .map(|col| {
                    SC::PackedVal::from_fn(|offset| {
                        let row = wrap(i_local_start + offset);
                        trace_lde.get(row, col)
                    })
                })
                .collect();
            let next: Vec<_> = (0..trace_lde.width())
                .map(|col| {
                    SC::PackedVal::from_fn(|offset| {
                        let row = wrap(i_next_start + offset);
                        trace_lde.get(row, col)
                    })
                })
                .collect();

            let accumulator = SC::PackedChallenge::zero();
            let mut folder = ProverConstraintFolder {
                main: TwoRowMatrixView {
                    local: &local,
                    next: &next,
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
