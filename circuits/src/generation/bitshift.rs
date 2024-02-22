use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::BitshiftView;
use crate::cpu::columns::CpuState;

fn filter_shift_trace<F: RichField>(cpu_trace: &[CpuState<F>]) -> impl Iterator<Item = u64> + '_ {
    cpu_trace
        .iter()
        .filter(|row| row.inst.ops.ops_that_shift().is_one())
        .map(|row| row.bitshift.amount.to_noncanonical_u64())
}

#[must_use]
pub fn generate_shift_amount_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> Vec<BitshiftView<F>> {
    let mut multiplicities = [0; 32];
    filter_shift_trace(cpu_trace).for_each(|amount| {
        multiplicities[usize::try_from(amount).expect("cast should succeed")] += 1;
    });
    (0..32u8)
        .map(|amount| {
            BitshiftView {
                executed: amount.into(),
                multiplicity: multiplicities[usize::from(amount)],
            }
            .map(F::from_canonical_u32)
        })
        .collect()
}
