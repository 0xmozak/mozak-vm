#![allow(dead_code)]

use mozak_sdk::common::types::ProgramIdentifier;
use mozak_sdk::native::OrderedEvents;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::plonk::config::GenericConfig;
use serde::{Deserialize, Serialize};

/// An attestion to the correct execution of a `MozakVM` program, denoted by its
/// [`ProgramIdentifier`](mozak_sdk::coretypes::ProgramIdentifier).
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Attestation {
    /// The ID of the program that this attestation is associated with.
    pub id: ProgramIdentifier,
    // TODO(bing): Attest to its commitment
    /// Public inputs to the execution of a `MozakVM` program, provided in the
    /// clear.
    pub public_tape: Vec<u8>,
    // TODO(bing): Attest to its commitment
    /// Events emitted during the execution of a `MozakVM` program, provided in
    /// the clear.
    pub event_tape: OrderedEvents,
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
    pub constituent_zs: Vec<Attestation>,
}
