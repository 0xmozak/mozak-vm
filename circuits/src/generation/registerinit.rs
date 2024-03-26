use mozak_runner::vm::ExecutionRecord;
use plonky2::hash::hash_types::RichField;

use crate::registerinit::columns::RegisterInit;
use crate::utils::pad_trace_with_default;

/// Generates a register init ROM trace
#[must_use]
// TODO: For tests, we don't always start at 0.
// TODO: unify with `init_register_trace` in `generation/register.rs`
pub fn generate_register_init_trace<F: RichField>(
    record: &ExecutionRecord<F>,
) -> Vec<RegisterInit<F>> {
    let first_state = record
        .executed
        .first()
        .map_or(&record.last_state, |row| &row.state);

    pad_trace_with_default(
        (0..32)
            .map(|i| RegisterInit {
                reg_addr: F::from_canonical_u8(i),
                value: F::from_canonical_u32(first_state.get_register_value(i)),
                is_looked_up: F::from_bool(i != 0),
            })
            .collect(),
    )
}

// TODO(Matthias): restore the tests from before https://github.com/0xmozak/mozak-vm/pull/1371
