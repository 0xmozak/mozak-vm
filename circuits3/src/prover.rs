use p3_challenger::{CanObserve, FieldChallenger};
use p3_commit::{Pcs, UnivariatePcs, UnivariatePcsWithLde};
use p3_field::{AbstractExtensionField, AbstractField, TwoAdicField};
use p3_matrix::dense::RowMajorMatrix;
use p3_matrix::MatrixRows;
use p3_uni_stark::{decompose_and_flatten, StarkConfig};
use p3_util::log2_strict_usize;

use crate::bitshift::stark::BitShiftStark;
use crate::generation::bitshift::generate_bitshift_trace;
use crate::generation::xor::generate_dummy_xor_trace;
use crate::quotient::quotient_values;
use crate::xor::stark::XorStark;

const XOR_TRACE_LEN_LOG: u32 = 10;
const NUM_STARKS: usize = 2;

/// Note that this is an incomplete prover. Mainly intended for experiment.
/// # Panics
/// This function will panic if the number of traces is not equal to the number
/// of Starks.
pub fn prove<SC: StarkConfig>(config: &SC, mut challenger: SC::Challenger) {
    // collect traces of each stark as Matrices
    let traces = [
        generate_dummy_xor_trace(XOR_TRACE_LEN_LOG),
        generate_bitshift_trace(),
    ];

    let pcs = config.pcs();

    // height of each trace matrix
    let degrees: [usize; NUM_STARKS] = traces
        .iter()
        .map(p3_matrix::Matrix::height)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    // I need to figure out
    let log_quotient_degrees = [1, 1];

    let log_degrees = degrees.map(log2_strict_usize);
    let g_subgroups = log_degrees.map(SC::Val::two_adic_generator);

    // commit to traces
    let (main_commit, main_data) = config.pcs().commit_batches(traces.to_vec());
    challenger.observe(main_commit.clone());
    let alpha: SC::Challenge = challenger.sample_ext_element();
    let mut main_trace_ldes = config.pcs().get_ldes(&main_data);

    let trace_lde_1 = main_trace_ldes.pop().unwrap();
    let log_stride_for_quotient = pcs.log_blowup() - log_quotient_degrees[0];
    let trace_lde_1_for_quotient = trace_lde_1.vertically_strided(1 << log_stride_for_quotient, 0);

    // quotients
    let mut quotients: Vec<RowMajorMatrix<SC::Val>> = vec![];

    let quotient_values_1 = quotient_values(
        config,
        &BitShiftStark,
        log_degrees[0],
        log_quotient_degrees[0],
        trace_lde_1_for_quotient,
        alpha,
    );

    let quotient_chunks_flattened = decompose_and_flatten(
        quotient_values_1,
        SC::Challenge::from_base(pcs.coset_shift()),
        log_quotient_degrees[0],
    );

    quotients.push(quotient_chunks_flattened);

    let trace_lde_2 = main_trace_ldes.pop().unwrap();
    let log_stride_for_quotient = pcs.log_blowup() - log_quotient_degrees[1];
    let trace_lde_2_for_quotient = trace_lde_2.vertically_strided(1 << log_stride_for_quotient, 0);

    let quotient_values_2 = quotient_values(
        config,
        &XorStark,
        log_degrees[1],
        log_quotient_degrees[1],
        trace_lde_2_for_quotient,
        alpha,
    );

    let quotient_chunks_flattened = decompose_and_flatten(
        quotient_values_2,
        SC::Challenge::from_base(pcs.coset_shift()),
        log_quotient_degrees[1],
    );

    quotients.push(quotient_chunks_flattened);

    let (_quotient_commit, quotient_data) = config.pcs().commit_batches(quotients.to_vec());

    let zeta: SC::Challenge = challenger.sample_ext_element();
    let zeta_and_next: [Vec<SC::Challenge>; 2] = g_subgroups.map(|g| vec![zeta, zeta * g]);
    let zeta_exp_quotient_degree: [Vec<SC::Challenge>; 2] =
        log_quotient_degrees.map(|log_deg| vec![zeta.exp_power_of_2(log_deg)]);
    let prover_data_and_points = [
        // TODO: Causes some errors, probably related to the fact that not all chips have
        // preprocessed traces? (&preprocessed_data, zeta_and_next.as_slice()),
        (&main_data, zeta_and_next.as_slice()),
        (&quotient_data, zeta_exp_quotient_degree.as_slice()),
    ];
    let (_openings, _opening_proof) =
        pcs.open_multi_batches(&prover_data_and_points, &mut challenger);
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
