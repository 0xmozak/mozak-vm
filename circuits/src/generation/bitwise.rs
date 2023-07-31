use bitfield::Bit;
use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::bitwise::columns::{BitwiseColumnsView, BitwiseExecutionColumnsView};
use crate::cpu::columns::CpuColumnsView;

fn filter_bitwise_trace<F: RichField>(
    step_rows: &[CpuColumnsView<F>],
) -> impl Iterator<Item = &CpuColumnsView<F>> {
    step_rows.iter().filter(|row| {
        let ops = row.inst.ops;
        ops.and + ops.or + ops.xor + ops.sll + ops.srl != F::ZERO
        // TODO: add ops.sra once it's implemented.
    })
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::cast_possible_truncation)]
pub fn generate_bitwise_trace<F: RichField>(
    cpu_trace: &[CpuColumnsView<F>],
) -> Vec<BitwiseColumnsView<F>> {
    let mut trace: Vec<BitwiseColumnsView<F>> = vec![];
    for cpu_row in filter_bitwise_trace(cpu_trace) {
        let xor_a = cpu_row.xor.a;
        let xor_b = cpu_row.xor.b;
        let xor_out = cpu_row.xor.out;

        let to_bits = |val: F| {
            (0_usize..32)
                .map(|j| F::from_bool(val.to_canonical_u64().bit(j)))
                .collect_vec()
                .try_into()
                .unwrap()
        };

        let row = BitwiseColumnsView {
            is_execution_row: F::ONE,
            execution: BitwiseExecutionColumnsView {
                a: xor_a,
                b: xor_b,
                out: xor_out,
            },
            op1_limbs: to_bits(xor_a),
            op2_limbs: to_bits(xor_b),
            res_limbs: to_bits(xor_out),
        };
        trace.push(row);
    }

    trace.resize(
        trace.len().next_power_of_two(),
        BitwiseColumnsView::default(),
    );
    trace
}
