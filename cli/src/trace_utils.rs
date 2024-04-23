//! Util functions to help deal with individual stark traces

use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use mozak_circuits::test_utils::{C, D, F};
use mozak_sdk::common::types::Poseidon2Hash;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::plonk::config::{GenericHashOut, Hasher};
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;

/// Compute merkle cap of the trace, and return its hash
pub(crate) fn get_trace_commitment_hash<Row: IntoIterator<Item = F>>(
    trace: Vec<Row>,
    config: &StarkConfig,
) -> Poseidon2Hash {
    let trace_poly_values = trace_rows_to_poly_values(trace);
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;
    let trace_commitment = PolynomialBatch::<F, C, D>::from_values(
        trace_poly_values,
        rate_bits,
        false,
        cap_height,
        &mut TimingTree::default(),
        None,
    );
    let merkle_cap = trace_commitment.merkle_tree.cap;
    Poseidon2Hash(
        plonky2::hash::poseidon2::Poseidon2Hash::hash_no_pad(&merkle_cap.flatten())
            .to_bytes()
            .try_into()
            .unwrap(),
    )
}
