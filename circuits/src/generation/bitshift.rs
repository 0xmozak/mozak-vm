use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::{ShiftAmountView, FIXED_SHAMT_RANGE};
use crate::cpu::columns::CpuColumnsView;
use crate::utils::pad_trace_with_last;

fn filter_shift_trace<F: RichField>(
    step_rows: &[CpuColumnsView<F>],
) -> impl Iterator<Item = u64> + '_ {
    step_rows.iter().filter_map(|row| {
        (row.inst.ops.ops_that_shift().into_iter().sum::<F>() != F::ZERO)
            .then_some(row.bitshift.shamt.to_noncanonical_u64())
    })
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_shift_amount_trace<F: RichField>(
    cpu_trace: &[CpuColumnsView<F>],
) -> Vec<ShiftAmountView<F>> {
    pad_trace_with_last(
        filter_shift_trace(cpu_trace)
            .sorted()
            .merge_join_by(FIXED_SHAMT_RANGE, u64::cmp)
            .map(|dummy_or_executed| {
                ShiftAmountView {
                    is_executed: dummy_or_executed.is_left().into(),
                    executed: dummy_or_executed.into_left().into(),
                }
                .map(F::from_canonical_u64)
            })
            .collect(),
    )
}
