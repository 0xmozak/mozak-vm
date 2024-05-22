use std::borrow::Cow;
use std::convert::Infallible;
use std::iter::repeat_with;

use anyhow::{bail, Result};
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use itertools::{chain, merge_join_by, Either, Itertools};
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
        let program_branch = &self.program_branch_circuit;
        match (node, call_address) {
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
                        let proof = program_branch.prove(&merge, l_proof, Some(r_proof))?;
                    }
                    _ => {}
                }
            }
            _ => todo!(),
        }
        if let Some(call_address) = call_address {}
        Ok(())
    }

    fn merge_events_branch(
        &self,
        left: Option<EventNode>,
        right: Option<EventNode>,
    ) -> (Cow<MergeBranchProof>, Option<EventNode>) {
        match self.merge_events(left, right) {
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

    fn merge_events(
        &self,
        left: Option<EventNode>,
        right: Option<EventNode>,
    ) -> Option<(
        Either<Cow<MergeLeafProof>, Cow<MergeBranchProof>>,
        EventNode,
    )> {
        let leaf_circuit = &self.merge_leaf_circuit;
        let branch_circuit = &self.merge_branch_circuit;
        let (left, right) = match (left, right) {
            // Empty case
            (None, None) => return None,
            // Single case
            (left @ Some(_), right @ None) | (left @ None, right @ Some(_)) => {
                let proof = leaf_circuit
                    .prove(
                        branch_circuit,
                        left.as_ref().map(EventNode::hash),
                        right.as_ref().map(EventNode::hash),
                    )
                    .unwrap();
                return left.or(right).map(|e| (Either::Left(Cow::Owned(proof)), e));
            }
            (Some(left), Some(right)) => (left, right),
        };
        match (left, right) {
            (
                left @ EventNode::Leaf {
                    hash: l_hash,
                    event: l_event,
                },
                right @ EventNode::Leaf {
                    hash: r_hash,
                    event: r_event,
                },
            ) => {
                let mut l_address = BranchAddress::base(l_event.address);
                let mut r_address = BranchAddress::base(r_event.address);

                while l_address != r_address {
                    l_address = l_address.parent();
                    r_address = r_address.parent();
                }
                let address = l_address;

                return if l_event.address <= r_event.address {
                    let proof = leaf_circuit
                        .prove(branch_circuit, Some(l_hash), Some(r_hash))
                        .unwrap();
                    let hash = proof.merged_hash();
                    debug_assert_eq!(hash, Poseidon2Hash::two_to_one(l_hash, r_hash));
                    let event = EventNode::Branch {
                        address,
                        hash,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                    Some((Either::Left(Cow::Owned(proof)), event))
                } else {
                    let l_proof = leaf_circuit
                        .prove(branch_circuit, Some(l_hash), None)
                        .unwrap();
                    let r_proof = leaf_circuit
                        .prove(branch_circuit, None, Some(r_hash))
                        .unwrap();
                    let proof = branch_circuit.prove(&r_proof, &l_proof).unwrap();
                    let hash = proof.merged_hash();
                    debug_assert_eq!(hash, Poseidon2Hash::two_to_one(r_hash, l_hash));
                    let event = EventNode::Branch {
                        address,
                        hash,
                        left: Box::new(right),
                        right: Box::new(left),
                    };
                    Some((Either::Right(Cow::Owned(proof)), event))
                };
            },

            (
                left @ EventNode::Leaf {
                    hash: l_hash,
                    event: l_event,
                },
                EventNode::Branch {
                    address: r_address,
                    hash: r_hash,
                    left: r_child_left,
                    right: r_child_right,
                },
            ) => {
                use BranchAddressComparison::*;
                match BranchAddress::base(l_event.address).compare(&r_address) {
                    // Right should be above us
                    Equal | LeftSibling | RightSibling | LeftChild | RightChild => unreachable!(),
                    LeftParent => return self.merge_events(Some(left), Some(*r_child_left)),
                    RightParent => return self.merge_events(Some(left), Some(*r_child_right)),
                    RightCousin => todo!("merge proof with left-right"),
                    LeftCousin => todo!("merge proof with right-left"),
                }
            },
            _ => todo!("match the other leaf/branch pairings"),
        }

        todo!()
    }
}

pub struct TransactionAccumulator<'a> {
    aux: &'a AuxTransactionData,
    ongoing_tx: HashMap<OngoingTxKey, OngoingTx>,
    processed_txs: Option<ProcessedTx>,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct OngoingTxKey {
    cast_root: [F; 4],
    call_tape: [F; 4],
}

struct OngoingTx {
    nodes: OngoingTxNode,
}

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
            Self::Leaf { hash, .. } => *hash,
            Self::Branch { hash, .. } => *hash,
        }
    }
}

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

    pub fn ingest_program(
        &mut self,
        id: ProgramIdentifier,
        program_verifier: &VerifierOnlyCircuitData<C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        cast_list: Vec<ProgramIdentifier>,
        events: Vec<CanonicalEvent>,
        call_tape: [F; 4],
    ) -> Result<Option<&EventNode>> {
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
        let event_tree = reduce_tree_by_address(
            event_tree,
            |address| address.parent(),
            |a, l, r| {
                Ok::<_, Infallible>(EventNode::Branch {
                    address: *a,
                    hash: Poseidon2Hash::two_to_one(l.hash(), r.hash()),
                    left: Box::new(l),
                    right: Box::new(r),
                })
            },
        )?;
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
            call_tape,
            cast_root,
        };
        let tx_entry = match self.ongoing_tx.entry(key) {
            v @ Entry::Vacant(_) => {
                let proof = proof()?;
                let cast_list = merge_join_by(
                    cast_list.into_iter().map(OngoingTxNode::UnprocessedLeaf),
                    chain(repeat_with(|| None).take(call_index), [Some(
                        OngoingTxNode::ProcessedLeaf {
                            events: event_tree,
                            proof,
                        },
                    )]),
                    |_, r| r.is_some(),
                )
                .map(|v| v.map_right(Option::unwrap).into_inner());

                let nodes = reduce_tree(
                    cast_list.map(Box::new),
                    |x| *x,
                    Box::new,
                    |left, right| OngoingTxNode::UnprocessedBranch { left, right },
                )
                .unwrap();

                v.insert(OngoingTx { nodes })
            }
            Entry::Occupied(mut o) => {
                // TODO: fix to use address (MSB for direction)
                let mut call_index = call_index;
                let mut node = &mut o.get_mut().nodes;
                loop {
                    node = match node {
                        OngoingTxNode::UnprocessedBranch { left, right } =>
                            if call_index & 1 == 0 {
                                &mut *left
                            } else {
                                &mut *right
                            },
                        &mut OngoingTxNode::UnprocessedLeaf(pid) => {
                            if call_index != 0 || pid != id {
                                bail!("Bad traversal")
                            }
                            *node = OngoingTxNode::ProcessedLeaf {
                                events: event_tree,
                                proof: proof()?,
                            };
                            break;
                        }
                        _ => bail!("Bad traversal"),
                    };
                    call_index >>= 1;
                }
                o
            }
        };

        if let OngoingTxNode::ProcessedBranch { events, proof } = &tx_entry.get().nodes {}

        Ok(None)
    }
}
#[must_use]
pub fn reduce_tree_by_address<A: Clone + PartialEq, T, E>(
    mut i: Vec<(A, T)>,
    mut a: impl FnMut(A) -> A,
    mut f: impl FnMut(&A, T, T) -> Result<T, E>,
) -> Result<Option<(A, T)>, E> {
    while i.len() > 1 {
        i = i
            .into_iter()
            .chunk_by(|e| e.0.clone())
            .into_iter()
            .map(|(address, ts)| {
                let ts = ts.map(|x| Ok(x.1));
                let t = reduce_tree(ts, |x| x, |x| x, |l, r| f(&address, l?, r?)).unwrap()?;
                Ok((a(address), t))
            })
            .try_collect()?;
    }
    Ok(i.pop())
}

#[must_use]
pub fn reduce_tree<T, R>(
    i: impl IntoIterator<Item = T>,
    t: impl FnOnce(T) -> R,
    mut r: impl FnMut(R) -> T,
    mut f: impl FnMut(T, T) -> R,
) -> Option<R> {
    let mut i = i.into_iter();

    let mut stack: Vec<(R, usize)> = Vec::with_capacity(i.size_hint().0.ilog2() as usize + 1);
    let final_v = loop {
        let Some(v0) = i.next() else {
            break None;
        };
        let Some(v1) = i.next() else {
            break Some(v0);
        };
        let (mut v, mut c) = (f(v0, v1), 2);

        while let Some((pv, pc)) = stack.pop() {
            if pc != c {
                stack.push((pv, pc));
                break;
            }
            v = f(r(pv), r(v));
            c += pc;
        }
        stack.push((v, c));
    };

    let mut v = match (stack.pop(), final_v) {
        (None, None) => return None,
        (Some((pv, _)), None) => pv,
        (None, Some(v)) => return Some(t(v)),
        (Some((pv, _)), Some(v)) => f(r(pv), v),
    };
    while let Some((pv, _)) = stack.pop() {
        v = f(r(pv), r(v));
    }
    Some(v)
}
