use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::BitshiftView;
use crate::cpu::columns::CpuState;

fn filter_shift_trace<F: RichField>(cpu_trace: &[CpuState<F>]) -> impl Iterator<Item = u64> + '_ {
    cpu_trace
        .iter()
        .filter(|row| row.inst.ops.ops_that_shift().is_one())
        .map(|row| row.bitshift.amount.to_noncanonical_u64())
}

pub fn pad_trace<Row: Copy + core::fmt::Debug>(mut trace: Vec<Row>, default: Row) -> Vec<Row> {
    println!("trace.len(): {} {}", trace.len(), trace.len().next_power_of_two());
    trace.resize(trace.len().next_power_of_two(), default);
    println!("\n{trace:?}");
    trace
}

#[must_use]
pub fn generate_shift_amount_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> Vec<BitshiftView<F>> {
    print!("{:?}", cpu_trace[0]);
    pad_trace(
        filter_shift_trace(cpu_trace)
            .sorted()
            .merge_join_by(0..32, u64::cmp)
            .map(|executed_or_dummy| {
                BitshiftView {
                    is_executed: executed_or_dummy.has_left().into(),
                    executed: executed_or_dummy.into_left().into(),
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
