//! Util functions to help deal with individual stark traces

use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use plonky2::field::extension::Extendable;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;

/// Compute merkle cap of the trace
pub(crate) fn get_trace_merkle_cap<F, C, const D: usize, Row: IntoIterator<Item = F>>(
    trace: Vec<Row>,
    config: &StarkConfig,
) -> MerkleCap<F, C::Hasher>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
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
    trace_commitment.merkle_tree.cap
}
