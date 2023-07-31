use bitfield::Bit;
use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::bitwise::columns::{BitwiseColumnsView, BitwiseExecutionColumnsView};
use crate::cpu::columns::CpuColumnsView;

fn filter_bitwise_trace<F: RichField>(
    step_rows: &[CpuColumnsView<F>],
) -> impl Iterator<Item = &CpuColumnsView<F>> {
    step_rows
        .iter()
        .filter(|row| row.inst.ops.ops_that_use_xor().into_iter().sum::<F>() != F::ZERO)
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::cast_possible_truncation)]
pub fn generate_bitwise_trace<F: RichField>(
    cpu_trace: &[CpuColumnsView<F>],
) -> Vec<BitwiseColumnsView<F>> {
    let mut trace: Vec<BitwiseColumnsView<F>> = vec![];
    for cpu_row in filter_bitwise_trace(cpu_trace) {
        let a = cpu_row.xor.a;
        let b = cpu_row.xor.b;
        let out = cpu_row.xor.out;

        let to_bits = |val: F| {
            (0_usize..32)
                .map(|j| F::from_bool(val.to_canonical_u64().bit(j)))
                .collect_vec()
                .try_into()
                .unwrap()
        };

        let row = BitwiseColumnsView {
            is_execution_row: F::ONE,
            execution: BitwiseExecutionColumnsView { a, b, out },
            op1_limbs: to_bits(a),
            op2_limbs: to_bits(b),
            res_limbs: to_bits(out),
        };
        trace.push(row);
    }

    trace.resize(
        trace.len().next_power_of_two(),
        BitwiseColumnsView::default(),
    );
    trace
}
