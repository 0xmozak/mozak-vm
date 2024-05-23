use std::borrow::Cow;
use std::cmp::Ordering;
use std::convert::Infallible;

use anyhow::{bail, Result};
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use itertools::{merge_join_by, Either, EitherOrBoth, Itertools};
use mozak_recproofs::circuits::verify_program::core::ProgramPublicIndices;
use mozak_recproofs::circuits::{build_event_root, merge, verify_program, verify_tx};
use mozak_recproofs::{Event, EventType as ProofEventType};
use mozak_sdk::common::types::{CanonicalEvent, EventType as SdkEventType, ProgramIdentifier};
use mozak_sdk::core::constants::DIGEST_BYTES;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::HashOut;
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::plonk::circuit_data::{CircuitConfig, CommonCircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{GenericHashOut, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{AddressPath, BranchAddress, Dir};
use crate::block_proposer::BranchAddressComparison;
use crate::{C, D, F};

type EventLeafCircuit = build_event_root::LeafCircuit<F, C, D>;
type EventBranchCircuit = build_event_root::BranchCircuit<F, C, D>;

type ProgramLeafCircuit = verify_program::LeafCircuit<F, C, D>;
type ProgramBranchCircuit = verify_program::BranchCircuit<F, C, D>;

type ProgramLeafProof = verify_program::LeafProof<F, C, D>;
type ProgramBranchProof = verify_program::BranchProof<F, C, D>;

type TxLeafCircuit = verify_tx::LeafCircuit<F, C, D>;
type TxBranchCircuit = verify_tx::BranchCircuit<F, C, D>;

type TxLeafProof = verify_tx::LeafProof<F, C, D>;
type TxBranchProof = verify_tx::BranchProof<F, C, D>;

type MergeLeafCircuit = merge::LeafCircuit<F, C, D>;
type MergeBranchCircuit = merge::BranchCircuit<F, C, D>;

type MergeLeafProof = merge::LeafProof<F, C, D>;
type MergeBranchProof = merge::BranchProof<F, C, D>;
type MergeProof<'a> = Either<Cow<'a, MergeLeafProof>, Cow<'a, MergeBranchProof>>;

pub struct AuxTransactionData {
    event_leaf_circuit: EventLeafCircuit,
    event_branch_circuit: EventBranchCircuit,

    merge_leaf_circuit: MergeLeafCircuit,
    merge_branch_circuit: MergeBranchCircuit,

    program_leaf_circuit: ProgramLeafCircuit,
    program_branch_circuit: ProgramBranchCircuit,

    tx_leaf_circuit: TxLeafCircuit,
    tx_branch_circuit: TxBranchCircuit,

    empty_merge_leaf: MergeLeafProof,
    empty_merge_branch: MergeBranchProof,
}

impl AuxTransactionData {
    /// Create the auxillary transaction data. This includes all the circuits
    /// and dummy proofs. This only needs to be done once, as multiple
    /// `Transaction`s can use the same `AuxStateData`.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(
        config: &CircuitConfig,
        program_indices: &ProgramPublicIndices,
        program_common: &CommonCircuitData<F, D>,
    ) -> Self {
        let event_leaf_circuit = EventLeafCircuit::new(config);
        let event_branch_circuit = EventBranchCircuit::new(config, &event_leaf_circuit);

        let merge_leaf_circuit = MergeLeafCircuit::new(config);
        let merge_branch_circuit = MergeBranchCircuit::new(config, &merge_leaf_circuit);

        let program_leaf_circuit = ProgramLeafCircuit::new(
            config,
            program_indices,
            program_common,
            &event_branch_circuit,
        );
        let program_branch_circuit =
            ProgramBranchCircuit::new(config, &merge_branch_circuit, &program_leaf_circuit);

        let tx_leaf_circuit = TxLeafCircuit::new(config, &program_branch_circuit);
        let tx_branch_circuit =
            TxBranchCircuit::new(config, &merge_branch_circuit, &tx_leaf_circuit);

        let empty_merge_leaf = merge_leaf_circuit
            .prove(&merge_branch_circuit, None, None)
            .expect("Failed to construct leaf proof");

        let empty_merge_branch = merge_branch_circuit
            .prove(&empty_merge_leaf, &empty_merge_leaf)
            .expect("Failed to construct branch proof");

        AuxTransactionData {
            event_leaf_circuit,
            event_branch_circuit,
            merge_leaf_circuit,
            merge_branch_circuit,
            program_leaf_circuit,
            program_branch_circuit,
            tx_leaf_circuit,
            tx_branch_circuit,
            empty_merge_leaf,
            empty_merge_branch,
        }
    }

    fn insert_program(
        &self,
        node: &mut OngoingTxNode,
        call_address: Option<AddressPath<usize>>,
        events: Option<EventNode>,
        proof: impl FnOnce() -> Result<ProgramLeafProof>,
    ) -> Result<()> {
        if let Some(n) = self.insert_program_helper(node, call_address, events, proof)? {
            *node = n;
        }
        Ok(())
    }

    fn insert_program_helper(
        &self,
        node: &mut OngoingTxNode,
        call_address: Option<AddressPath<usize>>,
        events: Option<EventNode>,
        proof: impl FnOnce() -> Result<ProgramLeafProof>,
    ) -> Result<Option<OngoingTxNode>> {
        let program_branch = &self.program_branch_circuit;
        let new_node = match (node, call_address) {
            (OngoingTxNode::UnprocessedBranch { left, right }, Some(call_address)) => {
                let (call_address, next_node) = match call_address.next() {
                    (call_address, Dir::Left) => (call_address, left.as_mut()),
                    (call_address, Dir::Right) => (call_address, right.as_mut()),
                };
                self.insert_program(next_node, call_address, events, proof)?;

                match (left.as_mut(), right.as_mut()) {
                    (
                        OngoingTxNode::ProcessedLeaf {
                            events: l_events,
                            proof: l_proof,
                        },
                        OngoingTxNode::ProcessedLeaf {
                            events: r_events,
                            proof: r_proof,
                        },
                    ) => {
                        let (merge, events) =
                            self.merge_events_branch(l_events.take(), r_events.take());
                        let proof = program_branch.prove(&merge, l_proof, r_proof)?;
                        Some(OngoingTxNode::ProcessedBranch { events, proof })
                    }
                    _ => None,
                }
            }
            (OngoingTxNode::UnprocessedLeaf(_pid), None) => Some(OngoingTxNode::ProcessedLeaf {
                events,
                proof: proof()?,
            }),
            _ => todo!(),
        };

        Ok(new_node)
    }

    fn simple_merge(&self, left: EventNode, right: EventNode) -> (MergeLeafProof, EventNode) {
        let leaf_circuit = &self.merge_leaf_circuit;
        let branch_circuit = &self.merge_branch_circuit;

        let address = left.address().common_ancestor(right.address());

        let proof = leaf_circuit
            .prove(branch_circuit, Some(left.hash()), Some(right.hash()))
            .unwrap();

        let hash = proof.merged_hash();
        debug_assert_eq!(hash, Poseidon2Hash::two_to_one(left.hash(), right.hash()));

        let event = EventNode::Branch {
            address,
            hash,
            left: Box::new(left),
            right: Box::new(right),
        };

        (proof, event)
    }

    fn simple_merge_reverse(
        &self,
        left: EventNode,
        right: EventNode,
    ) -> (MergeBranchProof, EventNode) {
        let leaf_circuit = &self.merge_leaf_circuit;
        let branch_circuit = &self.merge_branch_circuit;

        let address = left.address().common_ancestor(right.address());

        let l_proof = leaf_circuit
            .prove(branch_circuit, Some(left.hash()), None)
            .unwrap();
        let r_proof = leaf_circuit
            .prove(branch_circuit, None, Some(right.hash()))
            .unwrap();
        let proof = branch_circuit.prove(&r_proof, &l_proof).unwrap();
        let hash = proof.merged_hash();
        debug_assert_eq!(hash, Poseidon2Hash::two_to_one(right.hash(), left.hash()));
        let event = EventNode::Branch {
            address,
            hash,
            left: Box::new(right),
            right: Box::new(left),
        };

        (proof, event)
    }

    fn merge_events_branch(
        &self,
        left: Option<EventNode>,
        right: Option<EventNode>,
    ) -> (Cow<MergeBranchProof>, Option<EventNode>) {
        match self.merge_maybe_events(left, right) {
            None => (Cow::Borrowed(&self.empty_merge_branch), None),
            Some((Either::Left(proof), event)) => {
                let proof = self
                    .merge_branch_circuit
                    .prove(&proof, &self.empty_merge_leaf)
                    .unwrap();
                (Cow::Owned(proof), Some(event))
            }
            Some((Either::Right(proof), event)) => (proof, Some(event)),
        }
    }

    fn merge_maybe_events(
        &self,
        left: Option<EventNode>,
        right: Option<EventNode>,
    ) -> Option<(MergeProof, EventNode)> {
        let leaf_circuit = &self.merge_leaf_circuit;
        let branch_circuit = &self.merge_branch_circuit;
        match (left, right) {
            // Empty case
            (None, None) => None,
            // Full case
            (Some(left), Some(right)) => Some(self.merge_events(left, right)),
            // Single case
            (left, right) => {
                let l_hash = left.as_ref().map(EventNode::hash);
                let r_hash = right.as_ref().map(EventNode::hash);
                let proof = leaf_circuit.prove(branch_circuit, l_hash, r_hash).unwrap();
                left.or(right).map(|e| (Either::Left(Cow::Owned(proof)), e))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn merge_events(&self, left: EventNode, right: EventNode) -> (MergeProof, EventNode) {
        use BranchAddressComparison::{
            Equal, LeftChild, LeftCousin, LeftParent, LeftSibling, RightChild, RightCousin,
            RightParent, RightSibling,
        };
        let leaf_circuit = &self.merge_leaf_circuit;
        let branch_circuit = &self.merge_branch_circuit;
        let comparison = left.address().compare(right.address());
        match (left, right, comparison) {
            // Unreachable states
            // LHS-Leaf and RHS-Leaf at different heights
            (EventNode::Leaf { .. }, EventNode::Leaf { .. }, LeftParent | RightParent | LeftChild | RightChild)
            // LHS-Leaf above RHS-Branch
            | (
                EventNode::Leaf { .. },
                EventNode::Branch { .. },
                Equal | LeftSibling | RightSibling | LeftChild | RightChild,
            )
            // LHS-Branch below RHS-Leaf
            | (
                EventNode::Branch { .. },
                EventNode::Leaf { .. },
                Equal | LeftSibling | RightSibling | LeftParent | RightParent,
            ) => unreachable!(),

            // Simple merges
            // LHS-Leaf equal to RHS-Leaf
            (left @ EventNode::Leaf { .. }, right @ EventNode::Leaf { .. }, Equal)
            // LHS to the left of RHS
            | (left, right, RightSibling | RightCousin) => {
                let (proof, event) = self.simple_merge(left, right);
                (Either::Left(Cow::Owned(proof)), event)
            }

            // Simple reverse merges
            // LHS to the right of RHS
            (left, right, LeftCousin | LeftSibling) => {
                let (proof, event) = self.simple_merge_reverse(left, right);
                (Either::Right(Cow::Owned(proof)), event)
            }

            // Right Recursions
            // LHS is a left-child of RHS-Branch
            (
                left,
                EventNode::Branch { left: right_child_l, right: right_child_r, address, .. },
                LeftParent,
            ) => {
                let (l_proof, l_event) = self.merge_events(left, *right_child_l);
                let r_proof = leaf_circuit
                    .prove(branch_circuit, None, Some(right_child_r.hash()))
                    .unwrap();
                let proof = match l_proof {
                    Either::Left(l_proof) => branch_circuit.prove(&l_proof, &r_proof),
                    Either::Right(l_proof) => branch_circuit.prove(&l_proof, &r_proof),
                }.unwrap();
                let event = EventNode::Branch {
                    address,
                    hash: proof.merged_hash(),
                    left: Box::new(l_event),
                    right: right_child_r,
                };

             (Either::Right(Cow::Owned(proof)), event)
            },
            // LHS is a right-child of RHS-Branch
            (
                left,
                EventNode::Branch { left: right_child_l, right: right_child_r, address, .. },
                RightParent,
            ) => {
                let l_proof = leaf_circuit
                    .prove(branch_circuit, None, Some(right_child_l.hash()))
                    .unwrap();
                let (r_proof, r_event) = self.merge_events(left, *right_child_r);
                let proof = match r_proof {
                    Either::Left(r_proof) => branch_circuit.prove(&l_proof, &r_proof),
                    Either::Right(r_proof) => branch_circuit.prove(&l_proof, &r_proof),
                }.unwrap();
                let event = EventNode::Branch {
                    address,
                    hash: proof.merged_hash(),
                    left: right_child_l,
                    right: Box::new(r_event),
                };

                (Either::Right(Cow::Owned(proof)), event)
            },


            // Left Recursions
            // LHS-Branch is a left-parent of RHS
            (
                EventNode::Branch { left: left_child_l, right: left_child_r, address, .. },
                right,
                LeftChild,
            ) => {
                let (l_proof, l_event) = self.merge_events(*left_child_l, right);
                let r_proof = leaf_circuit
                    .prove(branch_circuit, Some(left_child_r.hash()), None)
                    .unwrap();
                let proof = match l_proof {
                    Either::Left(l_proof) => branch_circuit.prove(&l_proof, &r_proof),
                    Either::Right(l_proof) => branch_circuit.prove(&l_proof, &r_proof),
                }.unwrap();
                let event = EventNode::Branch {
                    address,
                    hash: proof.merged_hash(),
                    left: Box::new(l_event),
                    right: left_child_r,
                };

                (Either::Right(Cow::Owned(proof)), event)
            },
            // LHS-Branch is a right-parent of RHS
            (
                EventNode::Branch { left: left_child_l, right: left_child_r, address, .. },
                right,
                RightChild,
            ) => {
                let l_proof = leaf_circuit
                    .prove(branch_circuit, Some(left_child_l.hash()), None)
                    .unwrap();
                let (r_proof, r_event) = self.merge_events(*left_child_r, right);
                let proof = match r_proof {
                    Either::Left(r_proof) => branch_circuit.prove(&l_proof, &r_proof),
                    Either::Right(r_proof) => branch_circuit.prove(&l_proof, &r_proof),
                }.unwrap();
                let event = EventNode::Branch {
                    address,
                    hash: proof.merged_hash(),
                    left: left_child_l,
                    right: Box::new(r_event),
                };

                (Either::Right(Cow::Owned(proof)), event)
            },

            (
                EventNode::Branch { left: left_child_l, right: left_child_r, address, .. },
                EventNode::Branch { left: right_child_l, right: right_child_r, .. },
                Equal
            ) => {
                let (l_proof, l_event) = self.merge_events(*left_child_l, *right_child_l);
                let (r_proof, r_event) = self.merge_events(*left_child_r, *right_child_r);

                let proof = match (l_proof, r_proof) {
                    (Either::Left(l), Either::Left(r)) => branch_circuit.prove(&l, &r),
                    (Either::Left(l), Either::Right(r)) => branch_circuit.prove(&l, &r),
                    (Either::Right(l), Either::Left(r)) => branch_circuit.prove(&l, &r),
                    (Either::Right(l), Either::Right(r)) => branch_circuit.prove(&l, &r),
                }.unwrap();
                let event = EventNode::Branch {
                    address,
                    hash: proof.merged_hash(),
                    left:  Box::new(l_event),
                    right: Box::new(r_event),
                };

                (Either::Right(Cow::Owned(proof)), event)
            },
        }
    }
}

pub struct TransactionAccumulator<'a> {
    aux: &'a AuxTransactionData,
    ongoing_tx: HashMap<OngoingTxKey, OngoingTx>,
    processed_txs: Option<ProcessedTx>,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct OngoingTxKey {
    cast_root: [F; 4],
    call_tape: [F; 4],
}

struct OngoingTx {
    nodes: OngoingTxNode,
}

#[derive(Debug)]
pub enum EventNode {
    Branch {
        address: BranchAddress,
        hash: HashOut<F>,
        left: Box<EventNode>,
        right: Box<EventNode>,
    },
    Leaf {
        hash: HashOut<F>,
        event: Event<F>,
    },
}

impl EventNode {
    fn hash(&self) -> HashOut<F> {
        match self {
            Self::Leaf { hash, .. } | Self::Branch { hash, .. } => *hash,
        }
    }

    fn address(&self) -> BranchAddress {
        match self {
            Self::Leaf { event, .. } => BranchAddress::base(event.address),
            Self::Branch { address, .. } => *address,
        }
    }
}

#[derive(Debug)]
enum OngoingTxNode {
    UnprocessedBranch {
        left: Box<OngoingTxNode>,
        right: Box<OngoingTxNode>,
    },
    ProcessedBranch {
        events: Option<EventNode>,
        proof: ProgramBranchProof,
    },
    ProcessedLeaf {
        events: Option<EventNode>,
        proof: ProgramLeafProof,
    },
    UnprocessedLeaf(ProgramIdentifier),
}

enum ProcessedTx {
    Leaf {
        proof: TxLeafProof,
        events: Option<EventNode>,
    },
    Branch {
        proof: TxBranchProof,
        events: Option<EventNode>,
    },
}

fn convert_event_type(ty: SdkEventType) -> ProofEventType {
    match ty {
        SdkEventType::Write => ProofEventType::Write,
        SdkEventType::Ensure => ProofEventType::Ensure,
        SdkEventType::Read => ProofEventType::Read,
        SdkEventType::Create => ProofEventType::GiveOwner,
        SdkEventType::Delete => ProofEventType::TakeOwner,
    }
}

impl<'a> TransactionAccumulator<'a> {
    /// Create an empty accumulator
    #[must_use]
    pub fn new(aux: &'a AuxTransactionData) -> Self {
        Self {
            aux,
            ongoing_tx: HashMap::new(),
            processed_txs: None,
        }
    }

    /// Ingests a program, combining it with any of its previously ingested cast
    /// members. Returns `(key, true)` when the final cast member of a tx is
    /// ingested.
    ///
    /// # Errors
    ///
    /// Returns an error if the user data was invalid in some way.
    ///
    /// # Panics
    ///
    /// Panics if the circuit logic has a bug.
    #[allow(clippy::too_many_lines)]
    pub fn ingest_program(
        &mut self,
        id: ProgramIdentifier,
        program_verifier: &VerifierOnlyCircuitData<C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        cast_list: Vec<ProgramIdentifier>,
        events: &[CanonicalEvent],
        call_tape: [F; 4],
    ) -> Result<(OngoingTxKey, bool)> {
        let Some(call_index) = cast_list.iter().position(|&r| r == id) else {
            bail!("id {id:?} was not in cast list");
        };

        let events = events.iter().map(|e| Event {
            owner: id.0.to_u64s().map(F::from_noncanonical_u64),
            ty: convert_event_type(e.type_),
            address: u64::from_le_bytes(e.address.0),
            value: e.value.to_u64s().map(F::from_noncanonical_u64),
        });

        let event_tree = events
            .clone()
            .map(|e| {
                (BranchAddress::base(e.address), EventNode::Leaf {
                    hash: e.hash(),
                    event: e,
                })
            })
            .collect();
        let event_tree = reduce_tree_by_address(event_tree, BranchAddress::parent, |a, l, r| {
            Ok::<_, Infallible>(EventNode::Branch {
                address: *a,
                hash: Poseidon2Hash::two_to_one(l.hash(), r.hash()),
                left: Box::new(l),
                right: Box::new(r),
            })
        })?;
        let event_tree = event_tree.map(|x| x.1);

        // Delay the proof calculation
        let proof = || {
            let event_branch = &self.aux.event_branch_circuit;
            let events = events
                .map(|e| {
                    let proof = self.aux.event_leaf_circuit.prove(event_branch, e)?;
                    Ok::<_, anyhow::Error>((e.address, Either::Left(proof)))
                })
                .try_collect()?;

            let event_root_proof = reduce_tree_by_address(
                events,
                |address| address / 2,
                |_, l, r| {
                    match (l, r) {
                        (Either::Left(l), Either::Left(r)) => event_branch.prove(&l, &r),
                        (Either::Left(l), Either::Right(r)) => event_branch.prove(&l, &r),
                        (Either::Right(l), Either::Left(r)) => event_branch.prove(&l, &r),
                        (Either::Right(l), Either::Right(r)) => event_branch.prove(&l, &r),
                    }
                    .map(Either::Right)
                },
            )?;
            let event_root_proof = event_root_proof.map(|x| x.1);

            let storage;
            let event_root_proof = match event_root_proof.as_ref() {
                None => Err(id.0.to_u64s().map(F::from_noncanonical_u64)),
                Some(Either::Right(v)) => Ok(v),
                Some(Either::Left(v)) => {
                    storage = event_branch.prove_one(v)?;
                    Ok(&storage)
                }
            };

            self.aux.program_leaf_circuit.prove(
                &self.aux.program_branch_circuit,
                program_verifier,
                program_proof,
                event_root_proof,
            )
        };

        let cast = cast_list.iter().map(ProgramIdentifier::inner);
        let cast_root = reduce_tree(
            cast,
            |x| HashOut::from_bytes(&x),
            |x| x.to_bytes().try_into().unwrap(),
            |l, r| {
                const SIZE: usize = DIGEST_BYTES * 2;
                let mut chain = [0; SIZE];
                chain[0..SIZE].copy_from_slice(&l);
                chain[SIZE..].copy_from_slice(&r);
                let chain = chain.map(F::from_canonical_u8);

                Poseidon2Hash::hash_no_pad(&chain)
            },
        )
        .unwrap()
        .elements;

        let key = OngoingTxKey {
            cast_root,
            call_tape,
        };
        let tx_entry = match self.ongoing_tx.entry(key) {
            v @ Entry::Vacant(_) => {
                let proof = proof()?;
                let cast_list = merge_join_by(
                    cast_list
                        .into_iter()
                        .map(OngoingTxNode::UnprocessedLeaf)
                        .enumerate(),
                    [(0, OngoingTxNode::ProcessedLeaf {
                        events: event_tree,
                        proof,
                    })],
                    |(i, _), _| {
                        if *i == call_index {
                            Ordering::Equal
                        } else {
                            Ordering::Less
                        }
                    },
                )
                .map(|v| match v {
                    EitherOrBoth::Left(v) => v.1,
                    EitherOrBoth::Both(_, v) => v.1,
                    EitherOrBoth::Right(_) => unreachable!(),
                });

                let nodes = reduce_tree(
                    cast_list.map(Box::new),
                    |x| *x,
                    Box::new,
                    |left, right| OngoingTxNode::UnprocessedBranch { left, right },
                )
                .unwrap();
                println!("todo remove {nodes:?}");

                v.insert(OngoingTx { nodes })
            }
            Entry::Occupied(mut o) => {
                self.aux.insert_program(
                    &mut o.get_mut().nodes,
                    AddressPath::path(call_index, cast_list.len().ilog2() as usize),
                    event_tree,
                    proof,
                )?;
                o
            }
        };

        let completed = if let OngoingTxNode::ProcessedBranch { .. } = &tx_entry.get().nodes {
            let leaf_circuit = &self.aux.tx_leaf_circuit;
            let branch_circuit = &self.aux.tx_branch_circuit;
            let tx = tx_entry.remove();
            let OngoingTxNode::ProcessedBranch { events, proof } = tx.nodes else {
                unreachable!()
            };

            let new_tx_events = events;
            let new_tx_proof = leaf_circuit.prove(branch_circuit, &proof).unwrap();

            self.processed_txs = Some(match self.processed_txs.take() {
                None => ProcessedTx::Leaf {
                    proof: new_tx_proof,
                    events: new_tx_events,
                },
                Some(ProcessedTx::Leaf { proof, events }) => {
                    let (merge_proof, events) = self.aux.merge_events_branch(events, new_tx_events);
                    let proof = branch_circuit
                        .prove(&merge_proof, &proof, Some(&new_tx_proof))
                        .unwrap();
                    ProcessedTx::Branch { proof, events }
                }
                Some(ProcessedTx::Branch { proof, events }) => {
                    let (merge_proof, events) = self.aux.merge_events_branch(events, new_tx_events);
                    let proof = branch_circuit
                        .prove(&merge_proof, &proof, Some(&new_tx_proof))
                        .unwrap();
                    ProcessedTx::Branch { proof, events }
                }
            });

            true
        } else if let OngoingTxNode::ProcessedLeaf { .. } = &tx_entry.get().nodes {
            let leaf_circuit = &self.aux.tx_leaf_circuit;
            let branch_circuit = &self.aux.tx_branch_circuit;
            let tx = tx_entry.remove();
            let OngoingTxNode::ProcessedLeaf { events, proof } = tx.nodes else {
                unreachable!()
            };

            let (merge_proof, events) = self.aux.merge_events_branch(events, None);
            let proof = self
                .aux
                .program_branch_circuit
                .prove_one(&merge_proof, &proof)
                .unwrap();

            let new_tx_events = events;
            let new_tx_proof = leaf_circuit.prove(branch_circuit, &proof).unwrap();

            self.processed_txs = Some(match self.processed_txs.take() {
                None => ProcessedTx::Leaf {
                    proof: new_tx_proof,
                    events: new_tx_events,
                },
                Some(ProcessedTx::Leaf { proof, events }) => {
                    let (merge_proof, events) = self.aux.merge_events_branch(events, new_tx_events);
                    let proof = branch_circuit
                        .prove(&merge_proof, &proof, Some(&new_tx_proof))
                        .unwrap();
                    ProcessedTx::Branch { proof, events }
                }
                Some(ProcessedTx::Branch { proof, events }) => {
                    let (merge_proof, events) = self.aux.merge_events_branch(events, new_tx_events);
                    let proof = branch_circuit
                        .prove(&merge_proof, &proof, Some(&new_tx_proof))
                        .unwrap();
                    ProcessedTx::Branch { proof, events }
                }
            });

            true
        } else {
            false
        };

        Ok((key, completed))
    }
}

/// Reduces a tree by merging all the items, grouped by their address,
/// then reducing their addresses
///
/// # Errors
///
/// Short circuits from any merge errors.
#[allow(clippy::missing_panics_doc)]
pub fn reduce_tree_by_address<A: Clone + PartialEq, T, E>(
    mut iter: Vec<(A, T)>,
    mut addr_inc: impl FnMut(A) -> A,
    mut merge: impl FnMut(&A, T, T) -> Result<T, E>,
) -> Result<Option<(A, T)>, E> {
    while iter.len() > 1 {
        iter = iter
            .into_iter()
            .chunk_by(|e| e.0.clone())
            .into_iter()
            .map(|(address, ts)| {
                let ts = ts.map(|x| Ok(x.1));
                let t = reduce_tree(ts, |x| x, |x| x, |l, r| merge(&address, l?, r?)).unwrap()?;
                Ok((addr_inc(address), t))
            })
            .try_collect()?;
    }
    Ok(iter.pop())
}

/// Reduces a tree by merging all the items
#[must_use]
pub fn reduce_tree<T, R>(
    iter: impl IntoIterator<Item = T>,
    make_ret: impl FnOnce(T) -> R,
    mut make_t: impl FnMut(R) -> T,
    mut merge: impl FnMut(T, T) -> R,
) -> Option<R> {
    let mut i = iter.into_iter();

    let mut stack: Vec<(R, usize)> = Vec::with_capacity(i.size_hint().0.ilog2() as usize + 1);
    let final_v = loop {
        let Some(v0) = i.next() else {
            break None;
        };
        let Some(v1) = i.next() else {
            break Some(v0);
        };
        let (mut v, mut c) = (merge(v0, v1), 2);

        while let Some((pv, pc)) = stack.pop() {
            if pc != c {
                stack.push((pv, pc));
                break;
            }
            v = merge(make_t(pv), make_t(v));
            c += pc;
        }
        stack.push((v, c));
    };

    let mut v = match (stack.pop(), final_v) {
        (None, None) => return None,
        (Some((pv, _)), None) => pv,
        (None, Some(v)) => return Some(make_ret(v)),
        (Some((pv, _)), Some(v)) => merge(make_t(pv), v),
    };
    while let Some((pv, _)) = stack.pop() {
        v = merge(make_t(pv), make_t(v));
    }
    Some(v)
}

#[cfg(test)]
mod test {
    use mozak_circuits::test_utils::fast_test_circuit_config;
    use mozak_recproofs::indices::{ArrayTargetIndex, BoolTargetIndex, HashOutTargetIndex};
    use mozak_recproofs::test_utils::make_fs;
    use once_cell::sync::Lazy;
    use plonky2::gates::noop::NoopGate;
    use plonky2::hash::hash_types::HashOutTarget;
    use plonky2::iop::target::{BoolTarget, Target};
    use plonky2::iop::witness::{PartialWitness, WitnessWrite};
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::circuit_data::CircuitData;

    use super::*;

    const FAST_CONFIG: bool = true;
    const CONFIG: CircuitConfig = if FAST_CONFIG {
        fast_test_circuit_config()
    } else {
        CircuitConfig::standard_recursion_config()
    };

    pub struct DummyCircuit {
        /// The program hash
        pub program_hash_val: [F; 4],

        /// The program hash
        pub program_hash: [Target; 4],

        /// The presence flag for the event root
        pub events_present: BoolTarget,

        /// The event root
        pub event_root: HashOutTarget,

        /// The call list
        pub call_list: [Target; 4],

        /// The cast list root
        pub cast_root: HashOutTarget,

        pub circuit: CircuitData<F, C, D>,
    }

    pub const ZERO_VAL: [F; 4] = [F::ZERO; 4];

    /// The hashes of the programs used
    pub const PROGRAM_HASHES: [[u64; 4]; 3] =
        [[31, 41, 59, 26], [53, 58, 97, 93], [23, 84, 62, 64]];

    impl DummyCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, program_id: impl Into<Option<usize>>) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let program_hash = builder.add_virtual_target_arr();
            let events_present = builder.add_virtual_bool_target_safe();
            let event_root = builder.add_virtual_hash();
            let call_list = builder.add_virtual_target_arr();
            let cast_root = builder.add_virtual_hash();

            builder.register_public_inputs(&program_hash);
            builder.register_public_input(events_present.target);
            builder.register_public_inputs(&event_root.elements);
            builder.register_public_inputs(&call_list);
            builder.register_public_inputs(&cast_root.elements);

            let program_hash_val = program_id
                .into()
                .map_or(ZERO_VAL, |pid| make_fs(PROGRAM_HASHES[pid]));

            let program_hash_calc = program_hash_val.map(|x| builder.constant(x));
            for (p, c) in program_hash.into_iter().zip(program_hash_calc) {
                builder.connect(p, c);
            }

            // Make sure we have enough gates to match.
            builder.add_gate(NoopGate, vec![]);
            while builder.num_gates() < (1 << 3) {
                builder.add_gate(NoopGate, vec![]);
            }

            let circuit = builder.build();

            Self {
                program_hash_val,
                program_hash,
                events_present,
                event_root,
                call_list,
                cast_root,
                circuit,
            }
        }

        pub fn get_indices(&self) -> ProgramPublicIndices {
            let public_inputs = &self.circuit.prover_only.public_inputs;
            ProgramPublicIndices {
                program_hash: ArrayTargetIndex::new(public_inputs, &self.program_hash),
                events_present: BoolTargetIndex::new(public_inputs, self.events_present),
                event_root: HashOutTargetIndex::new(public_inputs, self.event_root),
                call_list: ArrayTargetIndex::new(public_inputs, &self.call_list),
                cast_root: HashOutTargetIndex::new(public_inputs, self.cast_root),
            }
        }

        pub fn prove(
            &self,
            event_root: Option<HashOut<F>>,
            call_list: [F; 4],
            cast_root: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_bool_target(self.events_present, event_root.is_some());
            inputs.set_hash_target(self.event_root, event_root.unwrap_or_default());
            inputs.set_target_arr(&self.call_list, &call_list);
            inputs.set_hash_target(self.cast_root, cast_root);
            self.circuit.prove(inputs)
        }
    }

    pub static PROGRAM_M: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, None));
    pub static PROGRAM_0: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, 0));
    pub static PROGRAM_1: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, 1));
    pub static PROGRAM_2: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, 2));

    #[tested_fixture::tested_fixture(pub AUX)]
    fn build_aux() -> AuxTransactionData {
        let program_m_indices = PROGRAM_M.get_indices();
        assert_eq!(program_m_indices, PROGRAM_0.get_indices());
        assert_eq!(program_m_indices, PROGRAM_1.get_indices());
        assert_eq!(program_m_indices, PROGRAM_2.get_indices());

        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_0.circuit.common);
        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_1.circuit.common);
        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_2.circuit.common);

        AuxTransactionData::new(&CONFIG, &program_m_indices, &PROGRAM_M.circuit.common)
    }

    #[test]
    fn simple() {
        let mut txs = TransactionAccumulator::new(*AUX);
        let call_tape = make_fs([86, 7, 5, 309]);
        let proof = PROGRAM_M
            .prove(None, call_tape, PROGRAM_M.program_hash_val.into())
            .unwrap();
        let pid = ProgramIdentifier::default();
        let (_k, complete) = txs
            .ingest_program(
                pid,
                &PROGRAM_M.circuit.verifier_only,
                &proof,
                vec![pid],
                &[],
                call_tape,
            )
            .unwrap();
        assert!(complete);
    }
}
