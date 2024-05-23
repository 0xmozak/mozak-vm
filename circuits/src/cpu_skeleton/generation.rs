use itertools::chain;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use super::columns::CpuSkeleton;
use crate::utils::pad_trace_with_last;

#[must_use]
pub fn generate_cpu_skeleton_trace<F: RichField>(
    record: &ExecutionRecord<F>,
) -> Vec<CpuSkeleton<F>> {
    let ExecutionRecord {
        executed,
        last_state,
    } = record;
    let last_row = &[Row {
        state: last_state.clone(),
        // `Aux` has auxiliary information about an executed CPU cycle.
        // The last state is the final state after the last execution.  Thus naturally it has no
        // associated auxiliary execution information. We use a dummy aux to make the row
        // generation work, but we could refactor to make this unnecessary.
        ..executed.last().unwrap().clone()
    }];

    let trace = chain![executed, last_row]
        .map(|Row { state, .. }| CpuSkeleton {
            clk: F::from_noncanonical_u64(state.clk),
            pc: F::from_canonical_u32(state.get_pc()),
            is_running: F::from_bool(!state.halted),
        })
        .collect();
    log::trace!("trace {:?}", trace);
    pad_trace_with_last(trace)
}
