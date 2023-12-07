// TODO(daniel): remove all these
#![allow(unreachable_code)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(clippy::diverging_sub_expression)]

type Address = u32;
struct PartialAddress {
    height: usize,
    addr: Address,
}

impl PartialAddress {
    fn new(addr: Address) -> Self { Self { height: 0, addr } }

    fn next(&self) -> (Option<Self>, Dir) {
        let dir = if self.addr & 1 == 1 {
            Dir::Right
        } else {
            Dir::Left
        };

        if self.height == 0 {
            (None, dir)
        } else {
            let height = self.height - 1;
            let addr = self.addr >> 1;
            (Some(Self { height, addr }), dir)
        }
    }
}
pub type Object = [u8; 32];
type Digest = [u8; 32];

type UpdateProof = ();
type RecProof = ();
type RecLeafProof = ();

type Block = ();

enum Dir {
    Left,
    Right,
}

fn hash_deletion() -> Digest { todo!() }
fn hash_object(obj: &Object) -> Digest { todo!() }
fn hash_digests(left: Option<&Digest>, right: Option<&Digest>) -> Digest { todo!() }

struct Mutation {
    rec_proof: RecLeafProof,
    ty: MutationType,
}
enum MutationType {
    MutateData(Object),
    DeleteObject,
}

struct SparseMerkleBranch {
    height: usize,
    hash: Digest,
    mutated: Option<RecProof>,
    left: Option<Box<SparseMerkleNode>>,
    right: Option<Box<SparseMerkleNode>>,
}
struct SparseMerkleLeaf {
    hash: Digest,
    data: Option<Object>,
    mutation: Option<Mutation>,
}
enum SparseMerkleNode {
    Branch(SparseMerkleBranch),
    Leaf(SparseMerkleLeaf),
}

impl SparseMerkleNode {
    fn height(&self) -> Option<usize> {
        match self {
            Self::Branch(branch) => Some(branch.height),
            Self::Leaf(leaf) => None,
        }
    }

    fn digest(&self) -> &Digest {
        match self {
            Self::Branch(branch) => &branch.hash,
            Self::Leaf(leaf) => &leaf.hash,
        }
    }

    /// Create a pre-certified node with data from a previous block.
    ///
    /// The returned node should path down all the way to a leaf.
    fn up_to_date_from(address: Option<PartialAddress>, object: Object) -> Self {
        match address {
            Some(address) => {
                let (address, dir) = address.next();
                let child = Self::up_to_date_from(address, object);
                let height = child.height().map_or(0, |v| v + 1);

                SparseMerkleNode::Branch(match dir {
                    Dir::Left => SparseMerkleBranch {
                        height,
                        hash: hash_digests(Some(child.digest()), None),
                        mutated: None,
                        left: Some(Box::new(child)),
                        right: None,
                    },
                    Dir::Right => SparseMerkleBranch {
                        height,
                        hash: hash_digests(None, Some(child.digest())),
                        mutated: None,
                        left: None,
                        right: Some(Box::new(child)),
                    },
                })
            }
            None => SparseMerkleNode::Leaf(SparseMerkleLeaf {
                hash: hash_object(&object),
                data: Some(object),
                mutation: None,
            }),
        }
    }

    /// Create a mutation node for the proposed next block.
    ///
    /// The returned node should path down all the way to a leaf.
    fn mutation_from(address: Option<PartialAddress>, object: Object) -> Self {
        match address {
            Some(address) => {
                let (address, dir) = address.next();
                let child = Self::mutation_from(address, object);
                let height = child.height().map_or(0, |v| v + 1);

                SparseMerkleNode::Branch(match dir {
                    Dir::Left => SparseMerkleBranch {
                        height,
                        hash: hash_digests(Some(child.digest()), None),
                        left: Some(Box::new(child)),
                        right: None,
                        mutated: Some(todo!("queue job")),
                    },
                    Dir::Right => SparseMerkleBranch {
                        height,
                        hash: hash_digests(None, Some(child.digest())),
                        left: None,
                        right: Some(Box::new(child)),
                        mutated: Some(todo!("queue job")),
                    },
                })
            }
            None => SparseMerkleNode::Leaf(SparseMerkleLeaf {
                hash: hash_object(&object),
                data: None,
                mutation: Some(Mutation {
                    ty: MutationType::MutateData(object),
                    rec_proof: todo!("queue job"),
                }),
            }),
        }
    }

    /// Update the tree with pre-certified changes from a previous block.
    ///
    /// Do not call with mutations for our proposed next block!
    fn keep_object_up_to_date(&mut self, address: Option<PartialAddress>, object: Object) {
        match (self, address) {
            (Self::Branch(_), None) | (Self::Leaf(_), Some(_)) => panic!("bad address"),

            (Self::Branch(branch), Some(address)) => {
                debug_assert_eq!(branch.height, address.height);

                let (address, dir) = address.next();
                match (dir, &mut branch.left, &mut branch.right) {
                    (Dir::Left, None, _) => {
                        branch.left = Some(Box::new(Self::up_to_date_from(address, object)));
                    }
                    (Dir::Left, Some(left), _) => {
                        left.keep_object_up_to_date(address, object);
                    }
                    (Dir::Right, _, None) => {
                        branch.right = Some(Box::new(Self::up_to_date_from(address, object)));
                    }
                    (Dir::Right, _, Some(right)) => {
                        right.keep_object_up_to_date(address, object);
                    }
                }
                branch.hash = hash_digests(
                    branch.left.as_ref().map(|b| b.digest()),
                    branch.right.as_ref().map(|b| b.digest()),
                );
                if let Some(rec_proof) = branch.mutated {
                    todo!("queue for reproof")
                }
            }
            (Self::Leaf(leaf), None) => {
                if leaf.mutation.is_some() {
                    todo!("rollback relevant transaction responsible for this mutation");
                }
                leaf.hash = hash_object(&object);
                leaf.data = Some(object);
            }
        }
    }

    /// Update the tree with pre-certified deletions from a previous block.
    ///
    /// Do not call with mutations for our proposed next block!
    fn keep_deletion_up_to_date(&mut self, address: Option<PartialAddress>) -> bool {
        match (self, address) {
            (Self::Branch(_), None) | (Self::Leaf(_), Some(_)) => panic!("bad address"),

            (Self::Branch(branch), Some(address)) => {
                debug_assert_eq!(branch.height, address.height);

                let (address, dir) = address.next();
                match (dir, &mut branch.left, &mut branch.right) {
                    (Dir::Left, None, _) | (Dir::Right, _, None) => panic!("bad address"),
                    (Dir::Left, Some(left), _) =>
                        if left.keep_deletion_up_to_date(address) {
                            branch.left = None;
                        },
                    (Dir::Right, _, Some(right)) =>
                        if right.keep_deletion_up_to_date(address) {
                            branch.right = None;
                        },
                }
                branch.hash = hash_digests(
                    branch.left.as_ref().map(|b| b.digest()),
                    branch.right.as_ref().map(|b| b.digest()),
                );
                if let Some(rec_proof) = branch.mutated {
                    todo!("queue for reproof")
                }
                branch.left.is_none() && branch.right.is_none()
            }
            (Self::Leaf(leaf), None) => {
                if leaf.mutation.is_some() {
                    todo!("rollback relevant transaction responsible for this mutation");
                }
                true
            }
        }
    }

    /// Update the tree with mutations for the our proposed next block
    fn apply_mutation(&mut self, address: Option<PartialAddress>, object: Object) {
        match (self, address) {
            (Self::Branch(_), None) | (Self::Leaf(_), Some(_)) => panic!("bad address"),

            (Self::Branch(branch), Some(address)) => {
                debug_assert_eq!(branch.height, address.height);

                let (address, dir) = address.next();
                match (dir, &mut branch.left, &mut branch.right) {
                    (Dir::Left, None, _) => {
                        branch.left = Some(Box::new(Self::mutation_from(address, object)));
                    }
                    (Dir::Left, Some(left), _) => {
                        left.apply_mutation(address, object);
                    }
                    (Dir::Right, _, None) => {
                        branch.right = Some(Box::new(Self::mutation_from(address, object)));
                    }
                    (Dir::Right, _, Some(right)) => {
                        right.apply_mutation(address, object);
                    }
                }
                branch.hash = hash_digests(
                    branch.left.as_ref().map(|b| b.digest()),
                    branch.right.as_ref().map(|b| b.digest()),
                );
                branch.mutated = Some(todo!("queue for reproof"));
            }
            (Self::Leaf(leaf), None) => {
                debug_assert!(leaf.mutation.is_none(), "double mutation");

                leaf.hash = hash_object(&object);
                leaf.mutation = Some(Mutation {
                    rec_proof: todo!("queue for proof"),
                    ty: MutationType::MutateData(object),
                });
            }
        }
    }

    /// Update the tree with deletions for our proposed next block
    fn apply_deletion(&mut self, address: Option<PartialAddress>) {
        match (self, address) {
            (Self::Branch(_), None) | (Self::Leaf(_), Some(_)) => panic!("bad address"),

            (Self::Branch(branch), Some(address)) => {
                debug_assert_eq!(branch.height, address.height);

                let (address, dir) = address.next();
                match (dir, &mut branch.left, &mut branch.right) {
                    (Dir::Left, None, _) | (Dir::Right, _, None) => panic!("bad address"),
                    (Dir::Left, Some(left), _) => {
                        left.apply_deletion(address);
                    }
                    (Dir::Right, _, Some(right)) => {
                        right.apply_deletion(address);
                    }
                }
                branch.hash = hash_digests(
                    branch.left.as_ref().map(|b| b.digest()),
                    branch.right.as_ref().map(|b| b.digest()),
                );
                branch.mutated = Some(todo!("tag for reproof"));
            }
            (Self::Leaf(leaf), None) => {
                debug_assert!(leaf.mutation.is_none(), "double mutation");
                leaf.hash = hash_deletion();
                leaf.mutation = Some(Mutation {
                    rec_proof: todo!("queue for proof"),
                    ty: MutationType::DeleteObject,
                });
            }
        }
    }

    /// Our block has been fully accepted, go ahead and commit all mutations.
    fn finalize(&mut self) -> bool {
        match self {
            // Skip unmutated branches
            Self::Branch(branch) if branch.mutated.is_some() => false,
            Self::Branch(branch) => {
                let left = branch.left.as_mut().map_or(false, |b| b.finalize());
                if left {
                    branch.left = None;
                }
                let right = branch.right.as_mut().map_or(false, |b| b.finalize());
                if right {
                    branch.right = None;
                }
                branch.mutated = None;
                left && right
            }
            Self::Leaf(leaf) => match leaf.mutation.take().map(|m| m.ty) {
                None => false,
                Some(MutationType::DeleteObject) => true,
                Some(MutationType::MutateData(data)) => {
                    leaf.data = Some(data);
                    false
                }
            },
        }
    }
}

pub struct BlockProposer {
    tree: SparseMerkleNode,
}

impl BlockProposer {
    /// Adds an update to our proposed next block
    ///
    /// TODO(Daniel): this signature probably needs to change to have some way
    /// to callback with success or failure
    pub fn add_update(
        &mut self,
        _proof: UpdateProof,
        updated_objects: impl IntoIterator<Item = (Address, Object)>,
    ) {
        for (address, object) in updated_objects {
            self.tree
                .apply_mutation(Some(PartialAddress::new(address)), object);
        }
        todo!()
    }

    /// Process all the updates coming in from a previous block
    pub fn process_block(&mut self, _block: &Block) { todo!() }

    /// Propose a block
    #[must_use]
    pub fn propose_block(&self) -> Block { todo!() }

    /// Our block has been fully accepted, go ahead and commit everything and
    /// cleanup to prepare for the next batch of proposals.
    pub fn finalize_round(&mut self) { self.tree.finalize(); }
}
