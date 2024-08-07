use std::iter::successors;
use std::mem;
use std::ops::Add;

use itertools::Itertools;
use mozak_recproofs::circuits::state_update;
use plonky2::hash::hash_types::HashOut;
use plonky2::plonk::circuit_data::CircuitConfig;

use super::{Address, AddressPath, BranchAddress, Dir};
use crate::{C, D, F};

type Object = mozak_recproofs::Object<F>;
type LeafProof = state_update::LeafProof<F, C, D>;
type BranchProof = state_update::BranchProof<F, C, D>;
type LeafCircuit = state_update::LeafCircuit<F, C, D>;
type BranchCircuit = state_update::BranchCircuit<F, C, D>;

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Upsert(Object),
    Read,
    Delete,
}
#[allow(clippy::large_enum_variant)]
enum SparseMerkleNode {
    Branch(SparseMerkleBranch),
    Leaf(SparseMerkleLeaf),
}

struct SparseMerkleBranch {
    height: usize,
    proof: BranchProof,
    left: Option<Box<SparseMerkleNode>>,
    right: Option<Box<SparseMerkleNode>>,
}

struct SparseMerkleLeaf {
    proof: LeafProof,
    kind: LeafKind,
}

#[derive(Debug, Copy, Clone)]
enum LeafKind {
    DeleteEmptyLeaf,
    ReadEmptyLeaf,
    BeingCreated {
        new_object: Object,
    },
    Unused {
        object: Object,
    },
    BeingDeleted {
        object: Object,
    },
    BeingRead {
        object: Object,
    },
    BeingUpdated {
        old_object: Object,
        new_object: Object,
    },
}

enum FinalizeOutcome {
    Prune,
    Recalc,
    NoOp,
}

impl Add for FinalizeOutcome {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Recalc, _) | (_, Self::Recalc) => Self::Recalc,
            (Self::NoOp, _) | (_, Self::NoOp) => Self::NoOp,
            (Self::Prune, Self::Prune) => Self::Prune,
        }
    }
}

pub struct AuxStateData {
    max_tree_depth: usize,

    empty_leaf_hash: HashOut<F>,

    leaf_circuit: LeafCircuit,
    pub(super) branch_circuits: Vec<BranchCircuit>,

    empty_leaf_proof: LeafProof,
    empty_branch_proofs: Vec<BranchProof>,
}

impl AuxStateData {
    /// Create the auxiliary state data. This includes all the circuits
    /// and dummy proofs. This only needs to be done once, as multiple `State`s
    /// can use the same `AuxStateData` as long as it has sufficient max depth.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(config: &CircuitConfig, max_tree_depth: usize) -> Self {
        let empty_leaf = Object::default();
        let empty_leaf_hash = empty_leaf.hash();

        let leaf_circuit = LeafCircuit::new(config);
        let mut init = BranchCircuit::from_leaf(config, &leaf_circuit);
        let branch_circuits = (0..=max_tree_depth)
            .map(|_| {
                let next = BranchCircuit::from_branch(config, &init);
                mem::replace(&mut init, next)
            })
            .collect_vec();

        let empty_leaf_proof = leaf_circuit
            .prove(empty_leaf_hash, empty_leaf_hash, None)
            .expect("Failed to construct leaf proof");
        let empty_branch_proof = branch_circuits[0]
            .prove(&empty_leaf_proof, &empty_leaf_proof)
            .expect("Failed to construct branch proof");
        let empty_branch_proofs = successors(
            Some((empty_branch_proof, &branch_circuits[1..])),
            |(proof, circuits)| {
                circuits.split_first().map(|(circuit, circuits)| {
                    let proof = circuit
                        .prove(proof, proof)
                        .expect("Failed to construct branch proof");
                    (proof, circuits)
                })
            },
        )
        .map(|(proof, _)| proof)
        .collect_vec();

        Self {
            max_tree_depth,
            empty_leaf_hash,
            leaf_circuit,
            branch_circuits,
            empty_leaf_proof,
            empty_branch_proofs,
        }
    }

    fn apply_operation(&self, root: &mut SparseMerkleBranch, addr: Address, new: Operation) {
        let (path, dir) = addr.next(root.height);
        let _ = self.apply_operation_helper(root, addr, path, dir, new);
    }

    fn apply_operation_helper(
        &self,
        branch: &mut SparseMerkleBranch,
        addr: Address,
        path: Option<AddressPath>,
        dir: Dir,
        new: Operation,
    ) -> bool {
        let child = match dir {
            Dir::Left => &mut branch.left,
            Dir::Right => &mut branch.right,
        };
        let recalc;

        *child = Some(if let Some(mut child) = child.take() {
            recalc = match (path, &mut *child) {
                (Some(path), SparseMerkleNode::Branch(branch)) => {
                    let (path, dir) = path.next();
                    self.apply_operation_helper(branch, addr, path, dir, new)
                }
                (None, SparseMerkleNode::Leaf(leaf)) => self.apply_operation_leaf(leaf, addr, new),
                (_, _) => unreachable!("bad address or tree"),
            };

            child
        } else {
            recalc = true;

            Box::new(match path {
                Some(path) => SparseMerkleNode::Branch(self.create_branch_helper(addr, path, new)),
                None => SparseMerkleNode::Leaf(self.create_leaf_helper(addr, new)),
            })
        });

        if recalc {
            self.recalc_branch_helper(branch);
        }

        recalc
    }

    fn apply_operation_leaf(
        &self,
        leaf: &mut SparseMerkleLeaf,
        addr: Address,
        new: Operation,
    ) -> bool {
        let recalc;

        leaf.kind = match (&leaf.kind, new) {
            // Upgrade unused to Read
            (&LeafKind::Unused { object }, Operation::Read) => {
                recalc = true;
                let old_hash = leaf.proof.old();
                let new_leaf = self.being_read(addr, old_hash, object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // All other reads are a no-op
            (&k, Operation::Read) => {
                recalc = false;
                k
            }

            // Double delete and upgrade read to delete
            // are both no-ops
            (
                k @ (LeafKind::DeleteEmptyLeaf
                | LeafKind::ReadEmptyLeaf
                | LeafKind::BeingDeleted { .. }),
                Operation::Delete,
            ) => {
                recalc = false;
                *k
            }
            // Upgrade unused/read to Delete
            (&LeafKind::Unused { object } | &LeafKind::BeingRead { object }, Operation::Delete) => {
                recalc = true;
                let old_hash = leaf.proof.old();
                let new_leaf = self.being_deleted(addr, old_hash, object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // All other deletes are an error
            (k, Operation::Delete) => {
                panic!("attempted to delete after {k:?}")
            }

            // Upgrade empty read to create
            (LeafKind::ReadEmptyLeaf, Operation::Upsert(object)) => {
                recalc = true;
                let new_leaf = self.being_created(addr, object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // Upgrade unused/read to update
            (
                &LeafKind::Unused { object } | &LeafKind::BeingRead { object },
                Operation::Upsert(new_object),
            ) => {
                recalc = true;
                let old_hash = leaf.proof.old();
                let new_leaf = self.being_updated(addr, old_hash, object, new_object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // Ensure duplicate updates are identical
            (
                k @ (&LeafKind::BeingCreated { new_object }
                | &LeafKind::BeingUpdated { new_object, .. }),
                Operation::Upsert(object),
            ) => {
                assert_eq!(object, new_object, "double update");
                recalc = false;
                *k
            }
            // All other updates are an error
            (k, Operation::Upsert(object)) => {
                panic!("attempted to upsert {object:?} after {k:?}")
            }
        };

        recalc
    }

    fn recalc_branch_helper(&self, branch: &mut SparseMerkleBranch) {
        use SparseMerkleNode::{Branch, Leaf};

        let circuit = &self.branch_circuits[branch.height];
        let empty_leaf = &self.empty_leaf_proof;
        let empty_branch = branch
            .height
            .checked_sub(1)
            .and_then(|h| self.empty_branch_proofs.get(h));

        let left = branch.left.as_deref();
        let right = branch.right.as_deref();
        branch.proof = match (empty_branch, left, right) {
            // Empty node
            (None, None, None) => circuit.prove(empty_leaf, empty_leaf),
            (Some(empty), None, None) => circuit.prove(empty, empty),

            // Right node only
            (Some(empty), None, Some(Branch(r))) => circuit.prove(empty, &r.proof),
            (_, None, Some(Leaf(r))) => circuit.prove(empty_leaf, &r.proof),

            // Left node only
            (Some(empty), Some(Branch(l)), None) => circuit.prove(&l.proof, empty),
            (_, Some(Leaf(l)), None) => circuit.prove(&l.proof, empty_leaf),

            // Both nodes
            (_, Some(Branch(l)), Some(Branch(r))) => circuit.prove(&l.proof, &r.proof),
            (_, Some(Leaf(l)), Some(Leaf(r))) => circuit.prove(&l.proof, &r.proof),

            // Bad cases
            (None, _, _) => unreachable!("Missing branch circuit for child of {}", branch.height),
            (Some(_), Some(Branch(_)), Some(Leaf(_)))
            | (Some(_), Some(Leaf(_)), Some(Branch(_))) => unreachable!("Mismatched node types"),
        }
        .unwrap();
    }

    fn create_branch_helper(
        &self,
        addr: Address,
        path: AddressPath,
        new: Operation,
    ) -> SparseMerkleBranch {
        let (path, dir) = path.next();
        match path {
            None => {
                let leaf = self.create_leaf_helper(addr, new);
                let (left_proof, right_proof) = if dir == Dir::Left {
                    (&leaf.proof, &self.empty_leaf_proof)
                } else {
                    (&self.empty_leaf_proof, &leaf.proof)
                };

                let proof = self.branch_circuits[0]
                    .prove(left_proof, right_proof)
                    .unwrap();
                let leaf = Some(Box::new(SparseMerkleNode::Leaf(leaf)));

                let (left, right) = if dir == Dir::Left {
                    (leaf, None)
                } else {
                    (None, leaf)
                };

                SparseMerkleBranch {
                    height: 0,
                    proof,
                    left,
                    right,
                }
            }
            Some(path) => {
                let child = self.create_branch_helper(addr, path, new);
                let height = child.height + 1;
                let empty_child_proof = &self.empty_branch_proofs[height - 1];
                let (left_proof, right_proof) = if dir == Dir::Left {
                    (&child.proof, empty_child_proof)
                } else {
                    (empty_child_proof, &child.proof)
                };

                let proof = self.branch_circuits[height]
                    .prove(left_proof, right_proof)
                    .unwrap();
                let child = Some(Box::new(SparseMerkleNode::Branch(child)));

                let (left, right) = if dir == Dir::Left {
                    (child, None)
                } else {
                    (None, child)
                };

                SparseMerkleBranch {
                    height,
                    proof,
                    left,
                    right,
                }
            }
        }
    }

    fn empty_leaf(&self, addr: Address) -> LeafProof {
        self.leaf_circuit
            .prove(self.empty_leaf_hash, self.empty_leaf_hash, Some(addr.0))
            .unwrap()
    }

    fn read_empty_leaf(&self, addr: Address) -> SparseMerkleLeaf {
        SparseMerkleLeaf {
            kind: LeafKind::ReadEmptyLeaf,
            proof: self.empty_leaf(addr),
        }
    }

    fn delete_empty_leaf(&self, addr: Address) -> SparseMerkleLeaf {
        SparseMerkleLeaf {
            kind: LeafKind::DeleteEmptyLeaf,
            proof: self.empty_leaf(addr),
        }
    }

    fn being_created(&self, addr: Address, new_object: Object) -> SparseMerkleLeaf {
        let new_hash = new_object.hash();
        let proof = self
            .leaf_circuit
            .prove(self.empty_leaf_hash, new_hash, Some(addr.0))
            .unwrap();
        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingCreated { new_object },
        }
    }

    fn being_deleted(
        &self,
        addr: Address,
        old_hash: HashOut<F>,
        object: Object,
    ) -> SparseMerkleLeaf {
        let new_hash = self.empty_leaf_hash;
        let proof = self
            .leaf_circuit
            .prove(old_hash, new_hash, Some(addr.0))
            .unwrap();

        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingDeleted { object },
        }
    }

    fn being_read(&self, addr: Address, old_hash: HashOut<F>, object: Object) -> SparseMerkleLeaf {
        let proof = self
            .leaf_circuit
            .prove(old_hash, old_hash, Some(addr.0))
            .unwrap();

        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingRead { object },
        }
    }

    fn being_updated(
        &self,
        addr: Address,
        old_hash: HashOut<F>,
        object: Object,
        new_object: Object,
    ) -> SparseMerkleLeaf {
        let new_hash = new_object.hash();
        let proof = self
            .leaf_circuit
            .prove(old_hash, new_hash, Some(addr.0))
            .unwrap();
        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingUpdated {
                old_object: object,
                new_object,
            },
        }
    }

    fn create_leaf_helper(&self, addr: Address, new: Operation) -> SparseMerkleLeaf {
        match new {
            Operation::Delete => self.delete_empty_leaf(addr),
            Operation::Read => self.read_empty_leaf(addr),
            Operation::Upsert(object) => self.being_created(addr, object),
        }
    }

    fn finalize(&self, root: &mut SparseMerkleBranch) {
        self.finalize_branch(root, BranchAddress::root(root.height));
    }

    fn finalize_branch(
        &self,
        branch: &mut SparseMerkleBranch,
        addr: BranchAddress,
    ) -> FinalizeOutcome {
        let left_outcome = if let Some(mut left) = branch.left.take() {
            let outcome = match (&mut *left, addr.child(Dir::Left)) {
                (SparseMerkleNode::Branch(branch), Ok(addr)) => self.finalize_branch(branch, addr),
                (SparseMerkleNode::Leaf(leaf), Err(addr)) => self.finalize_leaf(leaf, addr),
                (_, _) => unreachable!("bad address or tree"),
            };
            if !matches!(outcome, FinalizeOutcome::Prune) {
                branch.left = Some(left);
            }
            outcome
        } else {
            FinalizeOutcome::Prune
        };

        let right_outcome = if let Some(mut right) = branch.right.take() {
            let outcome = match (&mut *right, addr.child(Dir::Right)) {
                (SparseMerkleNode::Branch(branch), Ok(addr)) => self.finalize_branch(branch, addr),
                (SparseMerkleNode::Leaf(leaf), Err(addr)) => self.finalize_leaf(leaf, addr),
                (_, _) => unreachable!("bad address or tree"),
            };
            if !matches!(outcome, FinalizeOutcome::Prune) {
                branch.right = Some(right);
            }
            outcome
        } else {
            FinalizeOutcome::Prune
        };

        let outcome = left_outcome + right_outcome;

        if let FinalizeOutcome::Recalc = outcome {
            self.recalc_branch_helper(branch);
        }

        outcome
    }

    fn finalize_leaf(&self, leaf: &mut SparseMerkleLeaf, _addr: Address) -> FinalizeOutcome {
        use LeafKind::{
            BeingCreated, BeingDeleted, BeingRead, BeingUpdated, DeleteEmptyLeaf, ReadEmptyLeaf,
            Unused,
        };
        let (old_hash, object) = match leaf.kind {
            Unused { .. } => return FinalizeOutcome::NoOp,
            DeleteEmptyLeaf | ReadEmptyLeaf | BeingDeleted { .. } => return FinalizeOutcome::Prune,
            BeingCreated { new_object } | BeingUpdated { new_object, .. } =>
                (leaf.proof.new(), new_object),
            BeingRead { object } => (leaf.proof.old(), object),
        };

        leaf.kind = Unused { object };

        leaf.proof = self.leaf_circuit.prove(old_hash, old_hash, None).unwrap();

        FinalizeOutcome::Recalc
    }
}

pub struct State<'a> {
    aux: &'a AuxStateData,
    root: SparseMerkleBranch,
}

impl<'a> State<'a> {
    /// Create the empty state data
    ///
    /// # Panics
    ///
    /// Will panic if `tree_depth` is unsupported by `aux`
    #[must_use]
    pub fn new(aux: &'a AuxStateData, tree_depth: usize) -> Self {
        assert!(tree_depth <= aux.max_tree_depth);
        let root = SparseMerkleBranch {
            height: tree_depth,
            proof: aux.empty_branch_proofs[tree_depth].clone(),
            left: None,
            right: None,
        };
        Self { aux, root }
    }

    pub fn apply_operation(&mut self, addr: Address, new: Operation) {
        self.aux.apply_operation(&mut self.root, addr, new);
    }

    pub fn finalize(&mut self) { self.aux.finalize(&mut self.root); }

    #[must_use]
    pub fn get_state(&self, addr: Address) -> (Option<&Object>, Option<&Object>) {
        let (path, dir) = addr.next(self.root.height);
        Self::get_state_helper(&self.root, path, dir)
    }

    fn get_state_helper(
        branch: &SparseMerkleBranch,
        path: Option<AddressPath>,
        dir: Dir,
    ) -> (Option<&Object>, Option<&Object>) {
        let child = match dir {
            Dir::Left => &branch.left,
            Dir::Right => &branch.right,
        };
        if let Some(child) = child.as_ref() {
            match (path, &**child) {
                (Some(path), SparseMerkleNode::Branch(branch)) => {
                    let (path, dir) = path.next();
                    Self::get_state_helper(branch, path, dir)
                }
                (None, SparseMerkleNode::Leaf(leaf)) => match &leaf.kind {
                    LeafKind::DeleteEmptyLeaf | LeafKind::ReadEmptyLeaf => (None, None),
                    LeafKind::BeingCreated { new_object } => (None, Some(new_object)),
                    LeafKind::Unused { object } | LeafKind::BeingRead { object } =>
                        (Some(object), Some(object)),
                    LeafKind::BeingDeleted { object } => (Some(object), None),
                    LeafKind::BeingUpdated {
                        old_object,
                        new_object,
                    } => (Some(old_object), Some(new_object)),
                },
                (_, _) => unreachable!("bad address or tree"),
            }
        } else {
            (None, None)
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::{AuxStateData, BranchProof, Operation, State};
    use crate::block_proposer::test_data::{CONFIG, SIMPLE_ADDRESS, SIMPLE_STATE_1};

    #[tested_fixture::tested_fixture(pub AUX_0)]
    fn build_aux_0() -> AuxStateData { AuxStateData::new(&CONFIG, 0) }

    #[tested_fixture::tested_fixture(pub AUX_8)]
    fn build_aux_8() -> AuxStateData { AuxStateData::new(&CONFIG, 8) }

    #[tested_fixture::tested_fixture(pub AUX_63)]
    fn build_aux_63() -> AuxStateData { AuxStateData::new(&CONFIG, 63) }

    fn simple(aux: &AuxStateData, v: usize) -> BranchProof {
        let mut state = State::new(aux, v);

        state.apply_operation(SIMPLE_ADDRESS, Operation::Read);
        let (before, after) = state.get_state(SIMPLE_ADDRESS);
        assert_eq!(before, None);
        assert_eq!(after, None);

        state.apply_operation(SIMPLE_ADDRESS, Operation::Upsert(SIMPLE_STATE_1));
        let (before, after) = state.get_state(SIMPLE_ADDRESS);
        assert_eq!(before, None);
        assert_eq!(after, Some(&SIMPLE_STATE_1));

        state.finalize();
        let (before, after) = state.get_state(SIMPLE_ADDRESS);
        assert_eq!(before, Some(&SIMPLE_STATE_1));
        assert_eq!(after, Some(&SIMPLE_STATE_1));

        state.apply_operation(SIMPLE_ADDRESS, Operation::Read);
        state.root.proof
    }

    #[tested_fixture::tested_fixture(pub SIMPLE_0)]
    fn simple_0() -> BranchProof { simple(*AUX_0, 0) }

    #[tested_fixture::tested_fixture(pub SIMPLE_8)]
    fn simple_8() -> BranchProof { simple(*AUX_8, 8) }

    #[tested_fixture::tested_fixture(pub SIMPLE_63)]
    fn simple_63() -> BranchProof { simple(*AUX_63, 63) }
}
