use crate::columns_view::columns_view_impl;

columns_view_impl!(TapeCommitments);

/// This stark table is used to store tape commitments
/// which we want to make public in final recursive proof.
/// Each commitment is stored as 32 bytes of hash, along with
/// their indices in each row. Different commitments can
/// be identified with their corresponding filter.
/// There is no definite order imposed on the rows of this
/// table,
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct TapeCommitments<T> {
    pub byte_with_index: CommitmentByteWithIndex<T>,
    pub multiplicity: T,
    pub is_castlist_commitment: T,
    pub is_event_tape_commitment: T,
}

columns_view_impl!(CommitmentByteWithIndex);

/// We store indices with the byte so that
/// we can do CTL against corresponding IOMemory stark,
/// while enforcing the original order in which bytes
/// are to be read.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CommitmentByteWithIndex<T> {
    pub byte: T,
    pub index: T,
}
