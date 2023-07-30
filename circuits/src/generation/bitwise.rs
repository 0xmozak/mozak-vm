use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::bitwise::columns::{BitwiseColumnsView, MAP};
use crate::columns_view::NumberOfColumns;
use crate::cpu::columns::{self as cpu_cols};

const NUM_BITWISE_COL: usize = BitwiseColumnsView::<()>::NUMBER_OF_COLUMNS;

#[must_use]
fn filter_bitwise_trace(step_rows: &[Row]) -> Vec<usize> {
    step_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            matches!(
                row.state.current_instruction().op,
                // TODO: Figure out a less error-prone way to check whether we need to deal with a
                // column.
                Op::AND | Op::OR | Op::XOR | Op::SLL | Op::SRL | Op::SRA
            )
        })
        .map(|(i, _row)| i)
        .collect()
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::cast_possible_truncation)]
pub fn generate_bitwise_trace<F: RichField>(
    step_rows: &[Row],
    cpu_trace: &[Vec<F>; cpu_cols::NUM_CPU_COLS],
) -> [Vec<F>; NUM_BITWISE_COL] {
    // TODO(Matthias): really use the new BitwiseColumnsView for generation, too.
    let filtered_step_rows = filter_bitwise_trace(step_rows);
    let trace_len = filtered_step_rows.len();
    let ext_trace_len = trace_len.next_power_of_two();
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; NUM_BITWISE_COL];
    for (i, clk) in filtered_step_rows.iter().enumerate() {
        let xor_a = cpu_trace[cpu_cols::MAP.xor_a][*clk];
        let xor_b = cpu_trace[cpu_cols::MAP.xor_b][*clk];
        let xor_out = cpu_trace[cpu_cols::MAP.xor_out][*clk];

        trace[MAP.execution.is_execution_row][i] = F::ONE;
        trace[MAP.execution.op1][i] = xor_a;
        trace[MAP.execution.op2][i] = xor_b;
        trace[MAP.execution.res][i] = xor_out;
        // TODO: make the CPU trace somehow pass the u32 values as well, not just the
        // field elements. So we don't have to reverse engineer them here.
        for (cols, u32_value) in [
            (MAP.op1_limbs, xor_a.to_canonical_u64() as u32),
            (MAP.op2_limbs, xor_b.to_canonical_u64() as u32),
            (MAP.res_limbs, xor_out.to_canonical_u64() as u32),
        ] {
            for (j, col) in cols.iter().enumerate() {
                trace[*col][i] = F::from_canonical_u32((u32_value >> j) & 0x0000_0001);
            }
        }
    }

    let trace_row_vecs = trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            NUM_BITWISE_COL,
            v.len()
        )
    });
    log::trace!("trace {:?}", trace_row_vecs);
    trace_row_vecs
}
