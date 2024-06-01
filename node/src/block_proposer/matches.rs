use std::borrow::Borrow;
use std::collections::HashMap;

use itertools::Either;
use mozak_recproofs::circuits::{accumulate_delta, match_delta};
use plonky2::plonk::circuit_data::CircuitConfig;

use super::Address;
use crate::{C, D, F};

type AccumulateLeafCircuit = accumulate_delta::LeafCircuit<F, C, D>;
type AccumulateBranchCircuit = accumulate_delta::BranchCircuit<F, C, D>;

type MatchLeafCircuit = match_delta::LeafCircuit<F, C, D>;
type MatchBranchCircuit = match_delta::BranchCircuit<F, C, D>;

type AccumulateLeafProof = accumulate_delta::LeafProof<F, C, D>;
type AccumulateBranchProof = accumulate_delta::BranchProof<F, C, D>;
type AccumulateProof = Either<AccumulateLeafProof, AccumulateBranchProof>;

pub struct AuxMatchesData {
    accumulate_leaf_circuit: AccumulateLeafCircuit,
    accumulate_branch_circuit: AccumulateBranchCircuit,

    match_leaf_circuit: MatchLeafCircuit,
    match_branch_circuit: MatchBranchCircuit,
}

impl AuxMatchesData {
    /// Create the auxillary matching data. This includes all the circuits
    /// and dummy proofs. This only needs to be done once, as multiple
    /// `Transaction`s can use the same `AuxStateData`.
    #[must_use]
    pub fn new(config: &CircuitConfig) -> Self {
        let accumulate_leaf_circuit = AccumulateLeafCircuit::new(config);
        let accumulate_branch_circuit =
            AccumulateBranchCircuit::new(config, &accumulate_leaf_circuit);

        let match_leaf_circuit = MatchLeafCircuit::new(config, &accumulate_branch_circuit);
        let match_branch_circuit = MatchBranchCircuit::new(config, &match_leaf_circuit);

        Self {
            accumulate_leaf_circuit,
            accumulate_branch_circuit,
            match_leaf_circuit,
            match_branch_circuit,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct OngoingTxKey {
    cast_root: [F; 4],
    call_tape: [F; 4],
}

pub struct Matches<Aux> {
    aux: Aux,
    ongoing_accum: HashMap<OngoingTxKey, OngoingAccum>,
    finalized_accum: HashMap<Address, AccumulateProof>,
}

struct OngoingAccum {
    events: HashMap<Address, AccumulateProof>,
}

impl<Aux: Borrow<AuxMatchesData>> Matches<Aux> {
    pub fn ingest_events(&mut self, key: OngoingTxKey) {
        let aux: &AuxMatchesData = self.aux.borrow();

        aux.accumulate_leaf_circuit.prove(
            &aux.accumulate_branch_circuit,
            address,
            event_owner,
            event_ty,
            event_value,
        )
    }
}
