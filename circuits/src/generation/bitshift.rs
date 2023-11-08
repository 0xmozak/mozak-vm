use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::{BitshiftView, MultiplicityView};
use crate::cpu::columns::CpuState;

fn filter_shift_trace<F: RichField>(cpu_trace: &[CpuState<F>]) -> impl Iterator<Item = u64> + '_ {
    cpu_trace
        .iter()
        .filter(|row| row.inst.ops.ops_that_shift().is_one())
        .map(|row| row.bitshift.amount.to_noncanonical_u64())
}

pub fn pad_trace<Row: Copy>(mut trace: Vec<Row>, default: Row) -> Vec<Row> {
    trace.resize(trace.len().next_power_of_two().max(4), default);
    trace
}

#[must_use]
pub fn generate_shift_amount_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> Vec<BitshiftView<F>> {
    let mut multiplicity = [0; 32];
    filter_shift_trace(cpu_trace).for_each(|amount| {
        multiplicity[usize::try_from(amount).expect("cast should succeed")] += 1;
    });
    (0..32u64)
        .map(|amount| {
            BitshiftView {
                bitshift: MultiplicityView {
                    executed: amount.into(),
                    multiplicity: multiplicity
                        [usize::try_from(amount).expect("cast should succeed")],
                },
            }
            .map(F::from_canonical_u64)
        })
        .collect()
}
