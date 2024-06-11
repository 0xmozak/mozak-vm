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

    /// Indicates that all events associated with a `key` have been ingested.
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
    #[must_use]
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

#[cfg(test)]
mod test {
    use mozak_circuits::test_utils::fast_test_circuit_config;
    use mozak_recproofs::test_utils::make_fs;
    use mozak_recproofs::Object;
    use mozak_sdk::common::types::{
        CanonicalEvent, EventType, Poseidon2Hash, ProgramIdentifier, StateAddress,
    };
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::{AuxMatchesData, Matches, F};
    use crate::block_proposer::OngoingTxKey;

    const FAST_CONFIG: bool = true;
    const CONFIG: CircuitConfig = if FAST_CONFIG {
        fast_test_circuit_config()
    } else {
        CircuitConfig::standard_recursion_config()
    };

    #[tested_fixture::tested_fixture(AUX)]
    fn build_aux() -> AuxMatchesData { AuxMatchesData::new(&CONFIG) }

    #[test]
    fn simple() {
        let seed = 42;
        let key = OngoingTxKey {
            call_tape: make_fs([86, 7, 5, 309]),
            cast_root: make_fs([314, 15, 2, 9]),
        };
        let id = ProgramIdentifier::new_from_rand_seed(seed);

        let address = StateAddress::new_from_rand_seed(seed + 1);
        let value = Poseidon2Hash::new_from_rand_seed(seed + 2);
        let events = [CanonicalEvent {
            address,
            type_: EventType::Read,
            value,
        }];
        let obj = Object {
            constraint_owner: make_fs([1, 2, 3, 4]),
            credits: F::from_canonical_u64(100),
            last_updated: F::from_canonical_u64(4),
            data: value.to_u64s().map(F::from_noncanonical_u64),
        };

        let mut matches = Matches::new(*AUX);
        matches.ingest_events(key, &id, &events);
        matches.ready_tx(key).unwrap();
        let proof = matches.finalize(5, |addr| {
            assert_eq!(addr.0, u64::from_le_bytes(address.0));
            (obj, obj)
        });

        assert_eq!(proof.block_height(), 5);
    }

    #[test]
    fn complex() {
        let seed = 42;
        let block_height = 7;
        let key_1 = OngoingTxKey {
            call_tape: make_fs([86, 7, 5, 309]),
            cast_root: make_fs([314, 15, 2, 9]),
        };
        let key_2 = OngoingTxKey {
            call_tape: make_fs([86, 7, 5, 309]),
            cast_root: make_fs([314, 15, 2, 9]),
        };
        let id_m = ProgramIdentifier::default();
        let id_1_a = ProgramIdentifier::new_from_rand_seed(seed);
        let id_1_b = ProgramIdentifier::new_from_rand_seed(seed + 1);
        let id_2 = ProgramIdentifier::new_from_rand_seed(seed + 2);

        let address_1 = StateAddress::new_from_rand_seed(seed + 3);
        let address_2 = StateAddress::new_from_rand_seed(seed + 4);

        let value_1 = Poseidon2Hash::new_from_rand_seed(seed + 5);
        let value_2 = Poseidon2Hash::new_from_rand_seed(seed + 6);
        let value_3 = Poseidon2Hash::new_from_rand_seed(seed + 7);

        let old_obj_1 = Object::default();
        let new_obj_1 = Object {
            constraint_owner: id_1_a.0.to_u64s().map(F::from_noncanonical_u64),
            credits: F::from_canonical_u64(0),
            last_updated: F::from_canonical_u64(block_height),
            data: value_1.to_u64s().map(F::from_noncanonical_u64),
        };

        let old_obj_2 = Object {
            constraint_owner: id_2.0.to_u64s().map(F::from_noncanonical_u64),
            credits: F::from_canonical_u64(0),
            last_updated: F::from_canonical_u64(block_height),
            data: value_2.to_u64s().map(F::from_noncanonical_u64),
        };
        let new_obj_2 = Object {
            data: value_3.to_u64s().map(F::from_noncanonical_u64),
            ..old_obj_2
        };

        let events_1_m = [
            CanonicalEvent {
                address: address_1,
                type_: EventType::GiveOwner,
                value: id_1_a.0,
            },
            CanonicalEvent {
                address: address_1,
                type_: EventType::Write,
                value: value_1,
            },
        ];
        let events_1_a = [
            CanonicalEvent {
                address: address_1,
                type_: EventType::Ensure,
                value: value_1,
            },
            CanonicalEvent {
                address: address_1,
                type_: EventType::TakeOwner,
                value: Poseidon2Hash::default(),
            },
        ];

        let events_1_b = [CanonicalEvent {
            address: address_2,
            type_: EventType::Read,
            value: value_2,
        }];

        let events_2 = [CanonicalEvent {
            address: address_2,
            type_: EventType::Write,
            value: value_3,
        }];

        let mut matches = Matches::new(*AUX);
        matches.ingest_events(key_1, &id_1_a, &events_1_a);
        matches.ingest_events(key_2, &id_2, &events_2);
        matches.ingest_events(key_1, &id_m, &events_1_m);
        matches.ready_tx(key_2).unwrap();
        matches.ingest_events(key_1, &id_1_b, &events_1_b);
        matches.ready_tx(key_1).unwrap();

        let proof = matches.finalize(block_height, |addr| {
            if addr.0 == u64::from_le_bytes(address_1.0) {
                return (old_obj_1, new_obj_1);
            }
            if addr.0 == u64::from_le_bytes(address_2.0) {
                return (old_obj_2, new_obj_2);
            }
            unreachable!()
        });

        assert_eq!(proof.block_height(), block_height);
    }
}
