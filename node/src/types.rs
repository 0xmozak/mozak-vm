#![allow(dead_code)]

use mozak_circuits::generation::memoryinit::generate_private_tape_init_trace;
use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use mozak_runner::elf::Program;
use mozak_sdk::common::types::ProgramIdentifier;
use mozak_sdk::native::OrderedEvents;
use plonky2::field::extension::Extendable;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::plonk::config::GenericConfig;
use plonky2::util::timing::TimingTree;
use serde::{Deserialize, Serialize};
use starky::config::StarkConfig;

/// Attestation provided opaquely.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(bound = "")]
pub struct OpaqueAttestation<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
> {
    /// Hash of the private inputs in the execution of a `MozakVM` program.
    pub private_tape_hash: MerkleCap<F, C::Hasher>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>
    OpaqueAttestation<F, C, D>
{
    pub fn from_program(program: &Program, config: &StarkConfig) -> Self {
        let trace = generate_private_tape_init_trace(&program);
        let poly_values = trace_rows_to_poly_values(trace);

        let rate_bits = config.fri_config.rate_bits;
        let cap_height = config.fri_config.cap_height;
        let trace_commitment = PolynomialBatch::<F, C, D>::from_values(
            poly_values,
            rate_bits,
            false, // blinding
            cap_height,
            &mut TimingTree::default(),
            None, // fft_root_table
        );
        Self {
            private_tape_hash: trace_commitment.merkle_tree.cap,
        }
    }
}

/// Attestation provided in the clear.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(bound = "")]
pub struct TransparentAttestation {
    // TODO(bing): Attest to its commitment
    /// Public inputs to the execution of a `MozakVM` program, provided in the
    /// clear.
    pub public_tape: Vec<u8>,
    // TODO(bing): Attest to its commitment
    /// Events emitted during the execution of a `MozakVM` program, provided in
    /// the clear.
    pub event_tape: OrderedEvents,
}

/// An attestion to the correct execution of a `MozakVM` program, denoted by its
/// [`ProgramIdentifier`](mozak_sdk::coretypes::ProgramIdentifier).
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Attestation<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// The ID of the program that this attestation is associated with.
    pub id: ProgramIdentifier,
    pub opaque: OpaqueAttestation<F, C, D>,
    pub transparent: TransparentAttestation,
}

/// The transaction sent across to sequencer
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Transaction<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    // TODO(bing): Attest to its commitment
    /// The cast list that declares all of the actors involved in this
    /// `Transaction`.
    pub cast_list: Vec<ProgramIdentifier>,
    /// The global call tape that all of the actors in the `cast_list` should be
    /// in agreement with.
    pub call_tape_hash: MerkleCap<F, C::Hasher>,
    /// The list of attestation(s) of the correct execution of the program(s)
    /// involved in this `Transaction`.
    pub constituent_zs: Vec<Attestation<F, C, D>>,
}
