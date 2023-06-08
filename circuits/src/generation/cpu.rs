use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;

pub fn generate_cpu_trace<F: RichField>(step_rows: Vec<Row>) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    let trace_len = step_rows.len();
    let ext_trace_len = if !trace_len.is_power_of_two() {
        trace_len.next_power_of_two()
    } else {
        trace_len
    };
    // TODO: May be need to duplicate last row?

    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; cpu_cols::NUM_CPU_COLS];
    for (i, s) in step_rows.iter().enumerate() {
        trace[cpu_cols::COL_CLK][i] = F::from_canonical_usize(s.state.clk);
        trace[cpu_cols::COL_PC][i] = F::from_canonical_u32(s.state.get_pc());

        trace[cpu_cols::COL_RS1][i] = F::from_canonical_u8(s.inst.data.rs1);
        trace[cpu_cols::COL_RS2][i] = F::from_canonical_u8(s.inst.data.rs2);
        trace[cpu_cols::COL_RD][i] = F::from_canonical_u8(s.inst.data.rd);

        for j in 0..32_usize {
            trace[cpu_cols::COL_START_REG + j][i] =
                F::from_canonical_u32(s.state.get_register_value(j));
        }

        match s.inst.op {
            Op::ADD => trace[cpu_cols::COL_SADD][i] = F::from_canonical_u8(1),
            _ => {}
        }
    }

    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cpu_cols::NUM_CPU_COLS,
            v.len()
        )
    })
}
