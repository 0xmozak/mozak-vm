use std::collections::{BTreeMap, HashMap};
use std::mem::take;

use anyhow::{bail, Result};
use itertools::{Either, Itertools};
use mozak_recproofs::circuits::accumulate_delta;
use mozak_recproofs::circuits::match_delta::{self, LeafWitnessValue};
use mozak_recproofs::Object;
use mozak_sdk::common::types::{CanonicalEvent, ProgramIdentifier};
use plonky2::field::types::PrimeField64;
use plonky2::plonk::circuit_data::CircuitConfig;

use super::{convert_event, reduce_tree_by_address, Address, BranchAddress, OngoingTxKey};
use crate::{C, D, F};

type AccumulateLeafCircuit = accumulate_delta::LeafCircuit<F, C, D>;
type AccumulateBranchCircuit = accumulate_delta::BranchCircuit<F, C, D>;

type MatchLeafCircuit = match_delta::LeafCircuit<F, C, D>;
type MatchBranchCircuit = match_delta::BranchCircuit<F, C, D>;

type AccumulateLeafProof = accumulate_delta::LeafProof<F, C, D>;
type AccumulateBranchProof = accumulate_delta::BranchProof<F, C, D>;
type AccumulateProof = Either<AccumulateLeafProof, AccumulateBranchProof>;

type MatchBranchProof = match_delta::BranchProof<F, C, D>;

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

pub struct Matches<'a> {
    aux: &'a AuxMatchesData,
    ongoing_accum: HashMap<OngoingTxKey, OngoingAccum>,
    ready_accum: BTreeMap<Address, AccumulateProof>,
}

#[derive(Default)]
struct OngoingAccum {
    events: HashMap<Address, AccumulateProof>,
}

impl<'a> Matches<'a> {
    /// Create an empty accumulator
    #[must_use]
    pub fn new(aux: &'a AuxMatchesData) -> Self {
        Self {
            aux,
            ongoing_accum: HashMap::new(),
            ready_accum: BTreeMap::new(),
        }
    }

    /// Ingests some events for an ongoing transaction
    ///
    /// # Panics
    ///
    /// Panics if the circuit logic has a bug.
    pub fn ingest_events(
        &mut self,
        key: OngoingTxKey,
        id: &ProgramIdentifier,
        events: &[CanonicalEvent],
    ) {
        let accum = self.ongoing_accum.entry(key).or_default();
        for event in events {
            use std::collections::hash_map::Entry;

            let event = convert_event(id, event);
            let proof = self
                .aux
                .accumulate_leaf_circuit
                .prove(
                    &self.aux.accumulate_branch_circuit,
                    event.address,
                    event.owner,
                    event.ty,
                    event.value,
                )
                .unwrap();
            match accum.events.entry(Address(event.address)) {
                Entry::Vacant(v) => {
                    v.insert(Either::Left(proof));
                }
                Entry::Occupied(mut o) => {
                    let proof = self
                        .aux
                        .accumulate_branch_circuit
                        .prove(o.get(), &proof)
                        .unwrap();
                    *o.get_mut() = Either::Right(proof);
                }
            }
        }
    }

    /// Indiciates that all events associated with a `key` have been ingested.
    ///
    /// The proofs associated with `key` will then be merged with other ready
    /// proofs that share the same addresses.
    ///
    /// # Errors
    ///
    /// Returns an error if ongoing transaction was not found.
    ///
    /// # Panics
    ///
    /// Panics if the circuit logic has a bug.
    pub fn ready_tx(&mut self, key: OngoingTxKey) -> Result<()> {
        let acc_branch = &self.aux.accumulate_branch_circuit;

        // TODO: leave a tomb stone to brick any future ingestion with this key
        let Some(accum) = self.ongoing_accum.remove(&key) else {
            bail!("Tx {key:?} was not found")
        };

        for (addr, proof) in accum.events {
            use std::collections::btree_map::Entry;

            match self.ready_accum.entry(addr) {
                Entry::Vacant(v) => {
                    v.insert(proof);
                }
                Entry::Occupied(mut o) => {
                    let proof = acc_branch.prove(o.get(), &proof).unwrap();
                    *o.get_mut() = Either::Right(proof);
                }
            }
        }

        Ok(())
    }

    /// Finalizes all ready transactions
    ///
    /// # Panics
    ///
    /// Panics if the circuit logic has a bug.
    pub fn finalize<O>(&mut self, block_height: u64, mut objs: O) -> MatchBranchProof
    where
        O: FnMut(Address) -> (Object<F>, Object<F>), {
        let leaf = &self.aux.match_leaf_circuit;
        let branch = &self.aux.match_branch_circuit;
        let acc_branch = &self.aux.accumulate_branch_circuit;

        let finalized = take(&mut self.ready_accum);
        let finalized = finalized
            .into_iter()
            .map(|(addr, acc_proof)| {
                let acc_proof = match acc_proof {
                    Either::Left(proof) => acc_branch.prove_one(&proof).unwrap(),
                    Either::Right(proof) => proof,
                };

                let (old, new) = objs(addr);
                let val = LeafWitnessValue {
                    block_height,
                    last_updated: new.last_updated.to_canonical_u64(),
                    old_owner: old.constraint_owner,
                    new_owner: new.constraint_owner,
                    old_data: old.data,
                    new_data: new.data,
                    old_credits: old.credits.to_canonical_u64(),
                    new_credits: new.credits.to_canonical_u64(),
                };
                let proof = leaf.prove(branch, &acc_proof, val).unwrap();

                (BranchAddress::base(addr.0), Either::Left(proof))
            })
            .collect_vec();

        let finalized = reduce_tree_by_address(
            finalized,
            |x| x.parent(1),
            |_, l, r| Either::Right(branch.prove(&l, &r).unwrap()),
        )
        .unwrap()
        .1;

        match finalized {
            Either::Left(proof) => branch.prove_one(&proof).unwrap(),
            Either::Right(proof) => proof,
        }
    }
}
