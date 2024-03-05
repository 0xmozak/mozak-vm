#![allow(dead_code)]

use mozak_sdk::coretypes::{Event, ProgramIdentifier};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::plonk::config::GenericConfig;

/// Attestation provided opaquely.
#[derive(Debug)]
pub struct OpaqueAttestation<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
> {
    /// Hash of the private inputs in the execution of a MozakVM program.
    pub private_tape_hash: MerkleCap<F, C::Hasher>,
}

/// Attestation provided in the clear.
#[derive(Debug)]
pub struct TransparentAttestation {
    // TODO(bing): Attest to its commitment
    /// Public inputs to the execution of a MozakVM program, provided in the
    /// clear.
    pub public_tape: Vec<u8>,
    // TODO(bing): Attest to its commitment
    /// Events emitted during the execution of a MozakVM program, provided in
    /// the clear.
    pub event_tape: Vec<Event>,
}

/// An attestion to the correct execution of a MozakVM program, denoted by its
/// [`ProgramIdentifier`](mozak_sdk::coretypes::ProgramIdentifier).
#[derive(Debug)]
pub struct Attestation<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// The ID of the program that this attestation is associated with.
    pub id: ProgramIdentifier,
    pub opaque: OpaqueAttestation<F, C, D>,
    pub transparent: TransparentAttestation,
}

/// The transaction sent across to sequencer
#[derive(Debug)]
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
