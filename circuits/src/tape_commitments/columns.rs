use mozak_sdk::core::constants::DIGEST_BYTES;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::public_sub_table::PublicSubTable;
use crate::stark::mozak_stark::{TableWithTypedOutput, TapeCommitmentsTable};

make_col_map!(TAPE_COMMITMENTS, TapeCommitments);
columns_view_impl!(TapeCommitments);

/// This stark table is used to store tape commitments
/// which we want to make public in final recursive proof.
/// Each commitment is stored as 32 bytes of hash, along with
/// their indices in each row. Different commitments can
/// be identified with their corresponding filter.
/// There is no definite order imposed on the rows of this
/// table,
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct TapeCommitments<T> {
    pub commitment_byte_row: CommitmentByteWithIndex<T>,
    pub castlist_commitment_tape_multiplicity: T,
    pub event_commitment_tape_multiplicity: T,
    pub is_castlist_commitment_tape_row: T,
    pub is_event_commitment_tape_row: T,
}
columns_view_impl!(CommitmentByteWithIndex);

/// We store indices with the byte so that
/// we can do CTL against corresponding
/// [`StorageDeviceStark`](crate::storage_device::stark::StorageDeviceStark),
/// stark, while enforcing the original order in which bytes
/// are to be read.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct CommitmentByteWithIndex<T> {
    pub byte: T,
    pub index: T,
}

pub type TapeCommitmentCTL<T> = CommitmentByteWithIndex<T>;

#[must_use]
pub fn lookup_for_castlist_commitment() -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    TapeCommitmentsTable::new(
        TapeCommitmentCTL {
            byte: TAPE_COMMITMENTS.commitment_byte_row.byte,
            index: TAPE_COMMITMENTS.commitment_byte_row.index,
        },
        TAPE_COMMITMENTS.castlist_commitment_tape_multiplicity,
    )
}

#[must_use]
pub fn lookup_for_event_tape_commitment() -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    TapeCommitmentsTable::new(
        TapeCommitmentCTL {
            byte: TAPE_COMMITMENTS.commitment_byte_row.byte,
            index: TAPE_COMMITMENTS.commitment_byte_row.index,
        },
        TAPE_COMMITMENTS.event_commitment_tape_multiplicity,
    )
}

#[must_use]
pub fn make_event_commitment_tape_public() -> PublicSubTable {
    PublicSubTable {
        table: TapeCommitmentsTable::new(
            vec![TAPE_COMMITMENTS.commitment_byte_row.byte],
            TAPE_COMMITMENTS.is_event_commitment_tape_row,
        ),
        num_rows: DIGEST_BYTES,
    }
}

#[must_use]
pub fn make_castlist_commitment_tape_public() -> PublicSubTable {
    PublicSubTable {
        table: TapeCommitmentsTable::new(
            vec![TAPE_COMMITMENTS.commitment_byte_row.byte],
            TAPE_COMMITMENTS.is_castlist_commitment_tape_row,
        ),
        num_rows: DIGEST_BYTES,
    }
}
