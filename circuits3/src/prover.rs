use p3_challenger::{CanObserve, FieldChallenger};
use p3_commit::{Pcs, UnivariatePcs};
use p3_field::TwoAdicField;
use p3_uni_stark::StarkConfig;
use p3_util::log2_strict_usize;

use crate::generation::bitshift::generate_bitshift_trace;
use crate::generation::xor::generate_dummy_xor_trace;

const XOR_TRACE_LEN_LOG: u32 = 10;
const NUM_STARKS: usize = 2;

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

    let zeta: SC::Challenge = challenger.sample_ext_element();
    let zeta_and_next: [Vec<SC::Challenge>; 2] =
        core::array::from_fn(|i| vec![zeta, zeta * g_subgroups[i]]);
    challenger.observe(commit.clone());
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
