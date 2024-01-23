//! Subcircuits for recursively proving the merge of two binary merkle trees
//!
//! These subcircuits are recursive, building on top of each other to
//! create the next level up of the merged merkle tree.

use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone)]
pub struct PublicIndices {
    /// The indices of each of the elements of the left hash
    pub left_hash: [usize; NUM_HASH_OUT_ELTS],
    /// The indices of each of the elements of the right hash
    pub right_hash: [usize; NUM_HASH_OUT_ELTS],
    /// The indices of each of the elements of the merged hash
    pub merged_hash: [usize; NUM_HASH_OUT_ELTS],
}
