use anyhow::{bail, Result};
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use itertools::Either;
use mozak_recproofs::circuits::build_event_root::{
    BranchCircuit as EventBranchCircuit, LeafCircuit as EventLeafCircuit,
};
use mozak_recproofs::circuits::merge::{
    BranchCircuit as MergeBranchCircuit, LeafCircuit as MergeLeafCircuit,
    LeafProof as MergeLeafProof,
};
use mozak_recproofs::circuits::verify_program::core::ProgramPublicIndices;
use mozak_recproofs::circuits::verify_program::{
    BranchCircuit as ProgramBranchCircuit, BranchProof as ProgramBranchProof,
    LeafCircuit as ProgramLeafCircuit, LeafProof as ProgramLeafProof,
};
use mozak_recproofs::circuits::verify_tx::{
    BranchCircuit as TxBranchCircuit, BranchProof as TxBranchProof, LeafCircuit as TxLeafCircuit,
    LeafProof as TxLeafProof,
};
use mozak_recproofs::{Event, EventType as ProofEventType};
use mozak_sdk::common::types::{
    CanonicalEvent, EventType as SdkEventType, Poseidon2Hash, ProgramIdentifier,
};
use mozak_sdk::core::constants::DIGEST_BYTES;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::HashOut;
use plonky2::hash::poseidon2::Poseidon2Hash as Plonky2Poseidon2Hash;
use plonky2::plonk::circuit_data::{CircuitConfig, CommonCircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{GenericHashOut, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use crate::{C, D, F};

pub struct AuxTransactionData {
    event_leaf_circuit: EventLeafCircuit<F, C, D>,
    event_branch_circuit: EventBranchCircuit<F, C, D>,

    merge_leaf_circuit: MergeLeafCircuit<F, C, D>,
    merge_branch_circuit: MergeBranchCircuit<F, C, D>,

    program_leaf_circuit: ProgramLeafCircuit<F, C, D>,
    program_branch_circuit: ProgramBranchCircuit<F, C, D>,

    tx_leaf_circuit: TxLeafCircuit<F, C, D>,
    tx_branch_circuit: TxBranchCircuit<F, C, D>,

    empty_merge_leaf: MergeLeafProof<F, C, D>,
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
        }
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
        address: u64,
        hash: HashOut<F>,
        left: Box<OngoingTxNode>,
        right: Box<OngoingTxNode>,
    },
    Leaf {
        hash: HashOut<F>,
        event: Event<F>,
    },
}

enum OngoingTxNode {
    UnprocessedBranch {
        left: Box<OngoingTxNode>,
        right: Box<OngoingTxNode>,
    },
    ProcessedBranch {
        events: Option<EventNode>,
        proof: ProgramBranchProof<F, C, D>,
    },
    ProcessedLeaf {
        events: Option<EventNode>,
        proof: ProgramLeafProof<F, C, D>,
    },
    UnprocessedLeaf(ProgramIdentifier),
}

enum ProcessedTx {
    Leaf {
        proof: TxLeafProof<F, C, D>,
        events: Option<EventNode>,
    },
    Branch {
        proof: TxBranchProof<F, C, D>,
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

        // Delay the proof calculation
        let proof = || {
            let event_branch = &self.aux.event_branch_circuit;
            let events = events.iter().map(|e| {
                let proof = self.aux.event_leaf_circuit.prove(event_branch, Event {
                    owner: id.0.to_u64s().map(F::from_noncanonical_u64),
                    ty: convert_event_type(e.type_),
                    address: u64::from_le_bytes(e.address.0),
                    value: e.value.to_u64s().map(F::from_noncanonical_u64),
                });
                proof.map(Either::Left)
            });
            let event_root_proof = reduce_tree(
                events,
                |x| x,
                |x| x,
                |l, r| {
                    match (l?, r?) {
                        (Either::Left(l), Either::Left(r)) => event_branch.prove(&l, &r),
                        (Either::Left(l), Either::Right(r)) => event_branch.prove(&l, &r),
                        (Either::Right(l), Either::Left(r)) => event_branch.prove(&l, &r),
                        (Either::Right(l), Either::Right(r)) => event_branch.prove(&l, &r),
                    }
                    .map(Either::Right)
                },
            );

            let storage;
            let event_root_proof = event_root_proof.transpose()?;
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

                Plonky2Poseidon2Hash::hash_no_pad(&chain)
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
                let cast_list = cast_list.into_iter().enumerate().map(|(i, p)| {
                    if i == call_index {
                        OngoingTxNode::ProcessedLeaf {
                            events: todo!(),
                            proof,
                        }
                    } else {
                        OngoingTxNode::UnprocessedLeaf(p)
                    }
                });
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
                                events: todo!(),
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

        Ok(None)
    }
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
