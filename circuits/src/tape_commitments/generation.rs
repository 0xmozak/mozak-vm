use itertools::{chain, Itertools};
use mozak_runner::state::StorageDeviceOpcode;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use crate::tape_commitments::columns::{CommitmentByteWithIndex, TapeCommitments};

#[must_use]
pub fn num_ecalls<F: RichField>(step_rows: &[Row<F>], which_tape: StorageDeviceOpcode) -> usize {
    step_rows
        .iter()
        .filter(|row| {
            row.aux
                .storage_device_entry
                .as_ref()
                .is_some_and(|io| io.op == which_tape)
        })
        .count()
}

#[must_use]
pub fn generate_tape_commitment_trace_with_op_code<F: RichField>(
    execution: &ExecutionRecord<F>,
    which_tape_commitment: StorageDeviceOpcode,
) -> Vec<TapeCommitments<F>> {
    // TODO: Maybe we should have better ways to identify Tapes which
    // refer to commitment?
    let tape = match which_tape_commitment {
        StorageDeviceOpcode::StoreCastListCommitmentTape =>
            &execution.last_state.cast_list_commitment_tape,
        StorageDeviceOpcode::StoreEventsCommitmentTape =>
            &execution.last_state.events_commitment_tape,
        _ => unreachable!(),
    };
    // theoretically, we have no restriction on number of ecalls made,
    // even though, on sdk side we use the ecall at most once
    let num_tape_commitment_ecalls = F::from_canonical_u32(
        num_ecalls(&execution.executed, which_tape_commitment)
            .try_into()
            .unwrap(),
    );

    let is_castlist_commitment_tape_row = F::from_bool(matches!(
        which_tape_commitment,
        StorageDeviceOpcode::StoreCastListCommitmentTape
    ));

    let is_event_commitment_tape_row = F::from_bool(matches!(
        which_tape_commitment,
        StorageDeviceOpcode::StoreEventsCommitmentTape
    ));

    let castlist_commitment_tape_multiplicity =
        is_castlist_commitment_tape_row * num_tape_commitment_ecalls;
    let event_commitment_tape_multiplicity =
        is_event_commitment_tape_row * num_tape_commitment_ecalls;

    tape.iter()
        .enumerate()
        .map(|(i, hash_byte)| TapeCommitments {
            commitment_byte_row: CommitmentByteWithIndex {
                byte: *hash_byte,
                index: u8::try_from(i).expect("index must lie between 0 and 31"),
            }
            .map(F::from_canonical_u8),
            event_commitment_tape_multiplicity,
            castlist_commitment_tape_multiplicity,
            is_castlist_commitment_tape_row,
            is_event_commitment_tape_row,
        })
        .collect_vec()
}

#[must_use]
pub fn generate_tape_commitments_trace<F: RichField>(
    execution: &ExecutionRecord<F>,
) -> Vec<TapeCommitments<F>> {
    let cast_list_commitment_trace = generate_tape_commitment_trace_with_op_code(
        execution,
        StorageDeviceOpcode::StoreCastListCommitmentTape,
    );
    log::trace!("{cast_list_commitment_trace:?}");
    let events_commitment_tape_trace = generate_tape_commitment_trace_with_op_code(
        execution,
        StorageDeviceOpcode::StoreEventsCommitmentTape,
    );
    log::trace!("{events_commitment_tape_trace:?}");
    // Note that the final trace length is 64, hence no need to pad.
    chain(cast_list_commitment_trace, events_commitment_tape_trace).collect_vec()
}
