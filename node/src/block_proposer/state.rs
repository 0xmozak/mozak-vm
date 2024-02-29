use std::mem;
use std::ops::Add;

use itertools::{chain, Itertools};
use mozak_circuits::recproof::state_update::{self, AddressPresent};
use plonky2::field::types::Field;
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::plonk::circuit_data::CircuitConfig;
use plonky2::plonk::config::{GenericConfig, Hasher, Poseidon2GoldilocksConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

pub const D: usize = 2;
pub type C = Poseidon2GoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ProgramHash(pub [F; 4]);

/// The unique address of this object is an implicit information
/// contained in the path to reach this object in state tree
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Object {
    /// Constraint-Owner is the only program which can mutate the fields of this
    /// object
    constraint_owner: ProgramHash,

    /// The block number at which this was last updated
    last_updated: F,

    /// Running credits for execution and paying rent
    credits: F,

    /// Serialized data object understandable and affectable by
    /// `constraint_owner`
    data: [F; 4],
}

impl Object {
    fn hash(&self) -> [F; 4] {
        let inputs = chain!(
            self.constraint_owner.0,
            [self.last_updated, self.credits],
            self.data,
        )
        .collect_vec();
        Poseidon2Hash::hash_no_pad(&inputs).elements
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Address(pub u64);

impl Address {
    fn next(self, height: usize) -> (Option<AddressPath>, Dir) {
        debug_assert!(self.0 <= (1 << height));
        AddressPath { height, addr: self }.next()
    }

    // Must be kept in sync with `state_update::LeafCircuit::new`
    fn summary_hash(self, old: [F; 4], new: [F; 4]) -> [F; 4] {
        let inputs = chain!([F::from_canonical_u64(self.0)], old, new,).collect_vec();
        Poseidon2Hash::hash_no_pad(&inputs).elements
    }
}

/// The remaining bits of an address to be consumed as one traverses down the
/// tree towards a leaf.
#[derive(Debug, Clone, Copy)]
struct AddressPath {
    height: usize,
    addr: Address,
}

impl AddressPath {
    fn next(mut self) -> (Option<Self>, Dir) {
        // look at the MSB for the current direction
        let bit = 1 << self.height;

        let dir = if self.addr.0 & bit != 0 {
            Dir::Right
        } else {
            Dir::Left
        };

        // Pop the MSB
        self.addr.0 &= bit - 1;

        if self.height == 0 {
            debug_assert_eq!(self.addr.0, 0);
            (None, dir)
        } else {
            self.height -= 1;
            (Some(self), dir)
        }
    }
}

/// A partial address which is constructed starting at the root and moving
/// downward, adding on one bit at a time based on a provided direction
#[derive(Debug, Clone, Copy)]
struct BranchAddress {
    height: usize,
    addr: Address,
}

impl BranchAddress {
    /// Initialize the `BranchAddress` to the root node
    fn root(height: usize) -> Self {
        Self {
            height,
            addr: Address(0),
        }
    }

    /// Move downward, adding a `0|1` bit based on the dir (`Left|Right`).
    /// If we've reached the bottom, return an `Address` instead
    fn child(mut self, dir: Dir) -> Result<Self, Address> {
        self.addr = Address(
            self.addr.0 << 1
                | match dir {
                    Dir::Left => 0,
                    Dir::Right => 1,
                },
        );
        if self.height == 0 {
            Err(self.addr)
        } else {
            self.height -= 1;
            Ok(self)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Upsert(Object),
    Read,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Left,
    Right,
}

#[allow(clippy::large_enum_variant)]
enum SparseMerkleNode {
    Branch(SparseMerkleBranch),
    Leaf(SparseMerkleLeaf),
}

struct SparseMerkleBranch {
    height: usize,
    proof: ProofWithPublicInputs<F, C, D>,
    old_hash: [F; 4],
    new_hash: [F; 4],
    summary_hash: [F; 4],
    left: Option<Box<SparseMerkleNode>>,
    right: Option<Box<SparseMerkleNode>>,
}

struct SparseMerkleLeaf {
    proof: ProofWithPublicInputs<F, C, D>,
    kind: LeafKind,
}

#[derive(Debug, Copy, Clone)]
enum LeafKind {
    DeleteEmptyLeaf {
        summary_hash: [F; 4],
    },
    ReadEmptyLeaf {
        summary_hash: [F; 4],
    },
    BeingCreated {
        new_hash: [F; 4],
        new_object: Object,
        summary_hash: [F; 4],
    },
    Unused {
        old_hash: [F; 4],
        object: Object,
    },
    BeingDeleted {
        old_hash: [F; 4],
        object: Object,
        summary_hash: [F; 4],
    },
    BeingRead {
        old_hash: [F; 4],
        object: Object,
        summary_hash: [F; 4],
    },
    BeingUpdated {
        old_hash: [F; 4],
        old_object: Object,
        new_hash: [F; 4],
        new_object: Object,
        summary_hash: [F; 4],
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
        #[allow(clippy::match_same_arms)]
        match (self, rhs) {
            (Self::Prune, Self::Prune) => Self::Prune,
            (Self::Prune, Self::Recalc) => Self::Recalc,
            (Self::Prune, Self::NoOp) => Self::NoOp,

            (Self::Recalc, Self::Prune) => Self::Recalc,
            (Self::Recalc, Self::Recalc) => Self::Recalc,
            (Self::Recalc, Self::NoOp) => Self::Recalc,

            (Self::NoOp, Self::Prune) => Self::NoOp,
            (Self::NoOp, Self::Recalc) => Self::Recalc,
            (Self::NoOp, Self::NoOp) => Self::NoOp,
        }
    }
}

#[allow(clippy::struct_field_names)]
struct LeafHashes<'a> {
    old_hash: &'a [F; 4],
    new_hash: &'a [F; 4],
    summary_hash: &'a [F; 4],
}

pub fn hash_branch(l: &[F; 4], r: &[F; 4]) -> [F; 4] {
    Poseidon2Hash::hash_no_pad(&[l[0], l[1], l[2], l[3], r[0], r[1], r[2], r[3]]).elements
}

pub struct AuxStateData {
    max_tree_depth: usize,

    empty_summary_hash: [F; 4],

    empty_leaf_hash: [F; 4],
    empty_branch_hashes: Vec<[F; 4]>,

    leaf_circuit: state_update::LeafCircuit<F, C, D>,
    branch_circuits: Vec<state_update::BranchCircuit<F, C, D>>,

    empty_leaf_proof: ProofWithPublicInputs<F, C, D>,
    empty_branch_proofs: Vec<ProofWithPublicInputs<F, C, D>>,
}

impl AuxStateData {
    pub fn new(config: &CircuitConfig, max_tree_depth: usize) -> Self {
        let empty_summary_hash = [F::ZERO; 4];
        let empty_leaf_hash = [F::ZERO; 4];

        let mut curr = empty_leaf_hash;
        let empty_branch_hashes = (0..=max_tree_depth)
            .map(|_| {
                curr = hash_branch(&curr, &curr);
                curr
            })
            .collect_vec();

        let leaf_circuit = state_update::LeafCircuit::<F, C, D>::new(config);
        let mut init = state_update::BranchCircuit::<F, C, D>::from_leaf(config, &leaf_circuit);
        let branch_circuits = (0..=max_tree_depth)
            .map(|_| {
                let next = state_update::BranchCircuit::<F, C, D>::from_branch(config, &init);
                mem::replace(&mut init, next)
            })
            .collect_vec();

        let empty_leaf_proof = leaf_circuit
            .prove(
                empty_leaf_hash.into(),
                empty_leaf_hash.into(),
                empty_summary_hash.into(),
                None,
            )
            .unwrap();
        let mut init = empty_leaf_proof.clone();
        let empty_branch_proofs = branch_circuits
            .iter()
            .zip(&empty_branch_hashes)
            .map(|(circuit, hash)| {
                let hash = (*hash).into();
                init = circuit
                    .prove(&init, &init, hash, hash, empty_summary_hash.into(), ())
                    .unwrap();
                init.clone()
            })
            .collect_vec();
        Self {
            max_tree_depth,
            empty_summary_hash,
            empty_leaf_hash,
            empty_branch_hashes,
            leaf_circuit,
            branch_circuits,
            empty_leaf_proof,
            empty_branch_proofs,
        }
    }

    fn apply_operation(&self, root: &mut SparseMerkleBranch, addr: Address, new: Operation) {
        let (path, dir) = addr.next(root.height);
        let _ = self.apply_operation_helper(
            root,
            addr,
            Some(BranchAddress::root(root.height)),
            path,
            dir,
            new,
        );
    }

    fn apply_operation_helper(
        &self,
        branch: &mut SparseMerkleBranch,
        addr: Address,
        branch_addr: Option<BranchAddress>,
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
                    self.apply_operation_helper(branch, addr, None, path, dir, new)
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
            self.recalc_branch_helper(branch, false, branch_addr);
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
            (&LeafKind::Unused { old_hash, object }, Operation::Read) => {
                recalc = true;
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
                k @ (LeafKind::DeleteEmptyLeaf { .. }
                | LeafKind::ReadEmptyLeaf { .. }
                | LeafKind::BeingDeleted { .. }),
                Operation::Delete,
            ) => {
                recalc = false;
                *k
            }
            // Upgrade unused/read to Delete
            (
                &LeafKind::Unused { old_hash, object }
                | &LeafKind::BeingRead {
                    old_hash, object, ..
                },
                Operation::Delete,
            ) => {
                recalc = true;
                let new_leaf = self.being_deleted(addr, old_hash, object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // All other deletes are an error
            (k, Operation::Delete) => {
                panic!("attempted to delete after {k:?}")
            }

            // Upgrade empty read to create
            (LeafKind::ReadEmptyLeaf { .. }, Operation::Upsert(object)) => {
                recalc = true;
                let new_leaf = self.being_created(addr, object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // Upgrade unused/read to update
            (
                &LeafKind::Unused { old_hash, object }
                | &LeafKind::BeingRead {
                    old_hash, object, ..
                },
                Operation::Upsert(new_object),
            ) => {
                recalc = true;
                let new_leaf = self.being_updated(addr, old_hash, object, new_object);
                leaf.proof = new_leaf.proof;
                new_leaf.kind
            }
            // Ensure duplicate updates are identical
            (
                k @ (&LeafKind::BeingCreated { new_object, .. }
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

    fn recalc_branch_helper(
        &self,
        branch: &mut SparseMerkleBranch,
        set_old: bool,
        addr: Option<BranchAddress>,
    ) {
        let (empty_hash, empty_proof) = self.get_empty_child_helper(branch.height);

        let ((left_hash, left_proof), (right_hash, right_proof), summary) =
            match (&branch.left, &branch.right) {
                (None, None) => (
                    (empty_hash, empty_proof),
                    (empty_hash, empty_proof),
                    self.empty_summary_hash,
                ),
                (None, Some(right)) => (
                    (empty_hash, empty_proof),
                    self.get_node_new_helper(right),
                    *self.get_summary_helper(right),
                ),
                (Some(left), None) => (
                    self.get_node_new_helper(left),
                    (empty_hash, empty_proof),
                    *self.get_summary_helper(left),
                ),
                (Some(left), Some(right)) => (
                    self.get_node_new_helper(left),
                    self.get_node_new_helper(right),
                    hash_branch(
                        self.get_summary_helper(left),
                        self.get_summary_helper(right),
                    ),
                ),
            };

        branch.new_hash = hash_branch(left_hash, right_hash);
        if set_old {
            branch.old_hash = branch.new_hash;
        }
        branch.summary_hash = summary;
        branch.proof = self.branch_circuits[branch.height]
            .prove(
                left_proof,
                right_proof,
                branch.old_hash.into(),
                branch.new_hash.into(),
                branch.summary_hash.into(),
                addr.map_or(AddressPresent::Implicit, |v| {
                    AddressPresent::Present(v.addr.0)
                }),
            )
            .unwrap();
    }

    fn get_node_new_helper<'a>(
        &'a self,
        node: &'a SparseMerkleNode,
    ) -> (&'a [F; 4], &'a ProofWithPublicInputs<F, C, D>) {
        match node {
            SparseMerkleNode::Branch(SparseMerkleBranch {
                proof, new_hash, ..
            }) => (new_hash, proof),
            SparseMerkleNode::Leaf(SparseMerkleLeaf { proof, kind }) =>
                (self.get_leaf_hashes(kind).new_hash, proof),
        }
    }

    fn get_summary_helper<'a>(&'a self, node: &'a SparseMerkleNode) -> &'a [F; 4] {
        match node {
            SparseMerkleNode::Branch(SparseMerkleBranch { summary_hash, .. }) => summary_hash,
            SparseMerkleNode::Leaf(SparseMerkleLeaf { kind, .. }) =>
                self.get_leaf_hashes(kind).summary_hash,
        }
    }

    fn get_empty_child_helper(&self, height: usize) -> (&[F; 4], &ProofWithPublicInputs<F, C, D>) {
        if height == 0 {
            (&self.empty_leaf_hash, &self.empty_leaf_proof)
        } else {
            (
                &self.empty_branch_hashes[height - 1],
                &self.empty_branch_proofs[height - 1],
            )
        }
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
                let LeafHashes {
                    new_hash: leaf_new,
                    summary_hash: &leaf_summary,
                    ..
                } = self.get_leaf_hashes(&leaf.kind);
                let ((left_leaf, left_proof), (right_leaf, right_proof)) = if dir == Dir::Left {
                    (
                        (leaf_new, &leaf.proof),
                        (&self.empty_leaf_hash, &self.empty_leaf_proof),
                    )
                } else {
                    (
                        (&self.empty_leaf_hash, &self.empty_leaf_proof),
                        (leaf_new, &leaf.proof),
                    )
                };
                let old_hash = self.empty_branch_hashes[0];
                let new_hash = hash_branch(left_leaf, right_leaf);

                let proof = self.branch_circuits[0]
                    .prove(
                        left_proof,
                        right_proof,
                        old_hash.into(),
                        new_hash.into(),
                        leaf_summary.into(),
                        (),
                    )
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
                    old_hash,
                    new_hash,
                    summary_hash: leaf_summary,
                    left,
                    right,
                }
            }
            Some(path) => {
                let child = self.create_branch_helper(addr, path, new);
                let height = child.height + 1;
                let empty_child_hash = &self.empty_branch_hashes[height - 1];
                let child_new = &child.new_hash;
                let child_summary = child.summary_hash;
                let empty_child_proof = &self.empty_branch_proofs[height - 1];
                let ((left_child, left_proof), (right_child, right_proof)) = if dir == Dir::Left {
                    (
                        (child_new, &child.proof),
                        (empty_child_hash, empty_child_proof),
                    )
                } else {
                    (
                        (empty_child_hash, empty_child_proof),
                        (child_new, &child.proof),
                    )
                };
                let old_hash = self.empty_branch_hashes[height];
                let new_hash = hash_branch(left_child, right_child);

                let proof = self.branch_circuits[height]
                    .prove(
                        left_proof,
                        right_proof,
                        old_hash.into(),
                        new_hash.into(),
                        child_summary.into(),
                        (),
                    )
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
                    old_hash,
                    new_hash,
                    summary_hash: child_summary,
                    left,
                    right,
                }
            }
        }
    }

    fn get_leaf_hashes<'a>(&'a self, leaf: &'a LeafKind) -> LeafHashes<'a> {
        match leaf {
            LeafKind::DeleteEmptyLeaf { summary_hash }
            | LeafKind::ReadEmptyLeaf { summary_hash } => LeafHashes {
                old_hash: &self.empty_leaf_hash,
                new_hash: &self.empty_leaf_hash,
                summary_hash,
            },
            LeafKind::BeingCreated {
                new_hash,
                summary_hash,
                ..
            } => LeafHashes {
                old_hash: &self.empty_leaf_hash,
                new_hash,
                summary_hash,
            },
            LeafKind::Unused { old_hash, .. } => LeafHashes {
                old_hash,
                new_hash: old_hash,
                summary_hash: &self.empty_summary_hash,
            },
            LeafKind::BeingDeleted {
                old_hash,
                summary_hash,
                ..
            } => LeafHashes {
                old_hash,
                new_hash: &self.empty_leaf_hash,
                summary_hash,
            },
            LeafKind::BeingRead {
                old_hash,
                summary_hash,
                ..
            } => LeafHashes {
                old_hash,
                new_hash: old_hash,
                summary_hash,
            },
            LeafKind::BeingUpdated {
                old_hash,
                new_hash,
                summary_hash,
                ..
            } => LeafHashes {
                old_hash,
                new_hash,
                summary_hash,
            },
        }
    }

    fn empty_leaf(&self, addr: Address) -> ([F; 4], ProofWithPublicInputs<F, C, D>) {
        let summary_hash = addr.summary_hash(self.empty_leaf_hash, self.empty_leaf_hash);
        let proof = self
            .leaf_circuit
            .prove(
                self.empty_leaf_hash.into(),
                self.empty_leaf_hash.into(),
                summary_hash.into(),
                Some(addr.0),
            )
            .unwrap();
        (summary_hash, proof)
    }

    fn read_empty_leaf(&self, addr: Address) -> SparseMerkleLeaf {
        let (summary_hash, proof) = self.empty_leaf(addr);
        SparseMerkleLeaf {
            kind: LeafKind::ReadEmptyLeaf { summary_hash },
            proof,
        }
    }

    fn delete_empty_leaf(&self, addr: Address) -> SparseMerkleLeaf {
        let (summary_hash, proof) = self.empty_leaf(addr);
        SparseMerkleLeaf {
            kind: LeafKind::DeleteEmptyLeaf { summary_hash },
            proof,
        }
    }

    fn being_created(&self, addr: Address, new_object: Object) -> SparseMerkleLeaf {
        let new_hash = new_object.hash();
        let summary_hash = addr.summary_hash(self.empty_leaf_hash, new_hash);
        let proof = self
            .leaf_circuit
            .prove(
                self.empty_leaf_hash.into(),
                new_hash.into(),
                summary_hash.into(),
                Some(addr.0),
            )
            .unwrap();
        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingCreated {
                new_hash,
                new_object,
                summary_hash,
            },
        }
    }

    fn being_deleted(&self, addr: Address, old_hash: [F; 4], object: Object) -> SparseMerkleLeaf {
        let new_hash = self.empty_leaf_hash;
        let summary_hash = addr.summary_hash(old_hash, new_hash);
        let proof = self
            .leaf_circuit
            .prove(
                old_hash.into(),
                new_hash.into(),
                summary_hash.into(),
                Some(addr.0),
            )
            .unwrap();

        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingDeleted {
                old_hash,
                object,
                summary_hash,
            },
        }
    }

    fn being_read(&self, addr: Address, old_hash: [F; 4], object: Object) -> SparseMerkleLeaf {
        let summary_hash = addr.summary_hash(old_hash, old_hash);
        let proof = self
            .leaf_circuit
            .prove(
                old_hash.into(),
                old_hash.into(),
                summary_hash.into(),
                Some(addr.0),
            )
            .unwrap();

        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingRead {
                old_hash,
                object,
                summary_hash,
            },
        }
    }

    fn being_updated(
        &self,
        addr: Address,
        old_hash: [F; 4],
        object: Object,
        new_object: Object,
    ) -> SparseMerkleLeaf {
        let new_hash = new_object.hash();
        let summary_hash = addr.summary_hash(old_hash, new_hash);
        let proof = self
            .leaf_circuit
            .prove(
                old_hash.into(),
                new_hash.into(),
                summary_hash.into(),
                Some(addr.0),
            )
            .unwrap();
        SparseMerkleLeaf {
            proof,
            kind: LeafKind::BeingUpdated {
                old_hash,
                old_object: object,
                new_hash,
                new_object,
                summary_hash,
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
            self.recalc_branch_helper(branch, true, None);
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
            DeleteEmptyLeaf { .. } | ReadEmptyLeaf { .. } | BeingDeleted { .. } =>
                return FinalizeOutcome::Prune,
            BeingCreated {
                new_hash,
                new_object,
                ..
            }
            | BeingUpdated {
                new_hash,
                new_object,
                ..
            } => (new_hash, new_object),
            BeingRead {
                old_hash, object, ..
            } => (old_hash, object),
        };

        leaf.kind = Unused { old_hash, object };

        leaf.proof = self
            .leaf_circuit
            .prove(
                old_hash.into(),
                old_hash.into(),
                self.empty_summary_hash.into(),
                None,
            )
            .unwrap();

        FinalizeOutcome::Recalc
    }
}

pub struct State<'a> {
    aux: &'a AuxStateData,
    root: SparseMerkleBranch,
}

impl<'a> State<'a> {
    pub fn new(aux: &'a AuxStateData, tree_depth: usize) -> Self {
        assert!(tree_depth <= aux.max_tree_depth);
        let root = SparseMerkleBranch {
            height: tree_depth,
            proof: aux.empty_branch_proofs[tree_depth].clone(),
            old_hash: aux.empty_branch_hashes[tree_depth],
            new_hash: aux.empty_branch_hashes[tree_depth],
            summary_hash: aux.empty_summary_hash,
            left: None,
            right: None,
        };
        Self { aux, root }
    }

    pub fn apply_operation(&mut self, addr: Address, new: Operation) {
        self.aux.apply_operation(&mut self.root, addr, new);
    }

    pub fn finalize(&mut self) { self.aux.finalize(&mut self.root); }

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
                    LeafKind::DeleteEmptyLeaf { .. } | LeafKind::ReadEmptyLeaf { .. } =>
                        (None, None),
                    LeafKind::BeingCreated { new_object, .. } => (None, Some(new_object)),
                    LeafKind::Unused { object, .. } | LeafKind::BeingRead { object, .. } =>
                        (Some(object), Some(object)),
                    LeafKind::BeingDeleted { object, .. } => (Some(object), None),
                    LeafKind::BeingUpdated {
                        old_object,
                        new_object,
                        ..
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
mod test {
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::HashOut;
    use plonky2::hash::poseidon2::Poseidon2Hash;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::Hasher;

    use super::{Address, AuxStateData, Object, Operation, ProgramHash, State, F};

    pub fn hash_str(v: &str) -> HashOut<F> {
        let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
        Poseidon2Hash::hash_no_pad(&v)
    }

    #[test]
    fn tiny_tree() {
        let config = CircuitConfig::standard_recursion_config();
        let aux = AuxStateData::new(&config, 0);
        let mut state = State::new(&aux, 0);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1").elements;
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2").elements;

        state.apply_operation(Address(1), Operation::Read);
        let (before, after) = state.get_state(Address(1));
        assert_eq!(before, None);
        assert_eq!(after, None);

        let obj = Object {
            constraint_owner: ProgramHash(non_zero_hash_1),
            last_updated: F::from_canonical_u64(10),
            credits: F::from_canonical_u64(10000),
            data: non_zero_hash_2,
        };
        state.apply_operation(Address(1), Operation::Upsert(obj));
        let (before, after) = state.get_state(Address(1));
        assert_eq!(before, None);
        assert_eq!(after, Some(&obj));

        state.finalize();
        let (before, after) = state.get_state(Address(1));
        assert_eq!(before, Some(&obj));
        assert_eq!(after, Some(&obj));
    }

    #[test]
    fn small_tree() {
        let config = CircuitConfig::standard_recursion_config();
        let aux = AuxStateData::new(&config, 8);
        let mut state = State::new(&aux, 8);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1").elements;
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2").elements;

        state.apply_operation(Address(42), Operation::Read);
        let (before, after) = state.get_state(Address(42));
        assert_eq!(before, None);
        assert_eq!(after, None);

        let obj = Object {
            constraint_owner: ProgramHash(non_zero_hash_1),
            last_updated: F::from_canonical_u64(10),
            credits: F::from_canonical_u64(10000),
            data: non_zero_hash_2,
        };
        state.apply_operation(Address(42), Operation::Upsert(obj));
        let (before, after) = state.get_state(Address(42));
        assert_eq!(before, None);
        assert_eq!(after, Some(&obj));

        state.finalize();
        let (before, after) = state.get_state(Address(42));
        assert_eq!(before, Some(&obj));
        assert_eq!(after, Some(&obj));
    }

    #[test]
    #[ignore]
    fn big_tree() {
        let config = CircuitConfig::standard_recursion_config();
        let aux = AuxStateData::new(&config, 63);
        let mut state = State::new(&aux, 63);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1").elements;
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2").elements;

        state.apply_operation(Address(42 << 7), Operation::Read);
        let (before, after) = state.get_state(Address(42 << 7));
        assert_eq!(before, None);
        assert_eq!(after, None);

        let obj = Object {
            constraint_owner: ProgramHash(non_zero_hash_1),
            last_updated: F::from_canonical_u64(10),
            credits: F::from_canonical_u64(10000),
            data: non_zero_hash_2,
        };
        state.apply_operation(Address(42 << 7), Operation::Upsert(obj));
        let (before, after) = state.get_state(Address(42 << 7));
        assert_eq!(before, None);
        assert_eq!(after, Some(&obj));

        state.finalize();
        let (before, after) = state.get_state(Address(42 << 7));
        assert_eq!(before, Some(&obj));
        assert_eq!(after, Some(&obj));
    }
}
