use mozak_sdk::core::ecall::COMMITMENT_SIZE;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputs;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::public_sub_table::PublicSubTable;
use crate::stark::mozak_stark::{TableWithTypedOutput, TapeCommitmentsTable};

make_col_map!(TapeCommitments);
columns_view_impl!(TapeCommitments);
pub const EVENT_COMMITMENT_TAPE_OFFSET: usize = 25;
pub const CASTLIST_COMMITMENT_TAPE_OFFSET: usize = 57;

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
    pub commitment_byte_row: CommitmentByteWithIndex<T>,
    pub castlist_commitment_tape_multiplicity: T,
    pub event_commitment_tape_multiplicity: T,
    pub is_castlist_commitment_tape_row: T,
    pub is_event_commitment_tape_row: T,
}
columns_view_impl!(CommitmentByteWithIndex);

/// We store indices with the byte so that
/// we can do CTL against corresponding `IOMemory` stark,
/// while enforcing the original order in which bytes
/// are to be read.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CommitmentByteWithIndex<T> {
    pub byte: T,
    pub index: T,
}

columns_view_impl!(TapeCommitmentCTL);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct TapeCommitmentCTL<T> {
    pub byte: T,
    pub index: T,
}

#[must_use]
pub fn lookup_for_castlist_commitment() -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    TapeCommitmentsTable::new(
        TapeCommitmentCTL {
            byte: COL_MAP.commitment_byte_row.byte,
            index: COL_MAP.commitment_byte_row.index,
        },
        COL_MAP.castlist_commitment_tape_multiplicity,
    )
}

#[must_use]
pub fn lookup_for_event_tape_commitment() -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    TapeCommitmentsTable::new(
        TapeCommitmentCTL {
            byte: COL_MAP.commitment_byte_row.byte,
            index: COL_MAP.commitment_byte_row.index,
        },
        COL_MAP.event_commitment_tape_multiplicity,
    )
}

#[must_use]
pub fn make_event_commitment_tape_public() -> PublicSubTable {
    PublicSubTable {
        table: TapeCommitmentsTable::new(
            vec![COL_MAP.commitment_byte_row.byte],
            COL_MAP.is_castlist_commitment_tape_row,
        ),
        num_rows: 32,
    }
}

#[must_use]
pub fn make_castlist_commitment_tape_public() -> PublicSubTable {
    PublicSubTable {
        table: TapeCommitmentsTable::new(
            vec![COL_MAP.commitment_byte_row.byte],
            COL_MAP.is_event_commitment_tape_row,
        ),
        num_rows: 32,
    }
}

pub fn get_event_commitment_tape_from_proof<F, const D: usize, C>(
    proof: &ProofWithPublicInputs<F, C, D>,
) -> Vec<F>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    proof.public_inputs
        [EVENT_COMMITMENT_TAPE_OFFSET..EVENT_COMMITMENT_TAPE_OFFSET + COMMITMENT_SIZE]
        .to_vec()
}

pub fn get_castlist_commitment_tape_from_proof<F, const D: usize, C>(
    proof: &ProofWithPublicInputs<F, C, D>,
) -> Vec<F>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    proof.public_inputs
        [CASTLIST_COMMITMENT_TAPE_OFFSET..CASTLIST_COMMITMENT_TAPE_OFFSET + COMMITMENT_SIZE]
        .to_vec()
}
