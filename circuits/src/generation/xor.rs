use bitfield::Bit;
use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::utils::pad_trace_with_default;
use crate::xor::columns::{XorColumnsView, XorView};

fn filter_xor_trace<F: RichField>(
    step_rows: &[CpuState<F>],
) -> impl Iterator<Item = XorView<F>> + '_ {
    step_rows
        .iter()
        .filter_map(|row| (row.inst.ops.ops_that_use_xor() != F::ZERO).then_some(row.xor))
}

fn to_bits<F: RichField>(val: F) -> [F; u32::BITS as usize] {
    (0_usize..32)
        .map(|j| F::from_bool(val.to_canonical_u64().bit(j)))
        .collect_vec()
        .try_into()
        .unwrap()
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::cast_possible_truncation)]
pub fn generate_xor_trace<F: RichField>(cpu_trace: &[CpuState<F>]) -> Vec<XorColumnsView<F>> {
    pad_trace_with_default(
        filter_xor_trace(cpu_trace)
            .map(|execution| XorColumnsView {
                is_execution_row: F::ONE,
                execution,
                limbs: execution.map(to_bits),
            })
            .collect_vec(),
    )
}
