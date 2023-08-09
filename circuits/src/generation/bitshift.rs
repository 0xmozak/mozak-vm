use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::BitshiftView;
use crate::cpu::columns::CpuState;

fn filter_shift_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> impl Iterator<Item = u64> + '_ {
    cpu_trace.iter().filter_map(|row| {
        (row.inst.ops.ops_that_shift().into_iter().sum::<F>() != F::ZERO)
            .then_some(row.bitshift.amount.to_noncanonical_u64())
    })
}

pub fn pad_trace<Row: Copy>(mut trace: Vec<Row>, default: Row) -> Vec<Row> {
    trace.resize(trace.len().next_power_of_two(), default);
    trace
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_shift_amount_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> Vec<BitshiftView<F>> {
    pad_trace(
        filter_shift_trace(cpu_trace)
            .sorted()
            .merge_join_by(0..32, u64::cmp)
            .map(|dummy_or_executed| {
                BitshiftView {
                    is_executed: dummy_or_executed.is_left().into(),
                    executed: dummy_or_executed.into_left().into(),
                }
                .map(F::from_canonical_u64)
            })
            .collect(),
        BitshiftView {
            is_executed: false.into(),
            executed: 31_u64.into(),
        }
        .map(F::from_canonical_u64),
    )
}
