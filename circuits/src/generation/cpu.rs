use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::utils::from_;

pub fn pad_trace<F: RichField>(mut trace: Vec<Vec<F>>) -> Vec<Vec<F>> {
    let len = trace[0].len();
    if let Some(padded_len) = len.checked_next_power_of_two() {
        trace[cpu_cols::COL_CLK..cpu_cols::NUM_CPU_COLS]
            .iter_mut()
            .for_each(|col| {
                col.extend(vec![*col.last().unwrap(); padded_len - len]);
            });
    }
    trace
}

pub fn generate_cpu_trace<F: RichField>(step_rows: Vec<Row>) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    let trace_len = step_rows.len();
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; trace_len]; cpu_cols::NUM_CPU_COLS];
    for (i, s) in step_rows.iter().enumerate() {
        trace[cpu_cols::COL_CLK][i] = from_(s.state.clk);
        trace[cpu_cols::COL_PC][i] = from_(s.state.get_pc());

        trace[cpu_cols::COL_RS1][i] = from_(s.inst.data.rs1);
        trace[cpu_cols::COL_RS2][i] = from_(s.inst.data.rs2);
        trace[cpu_cols::COL_RD][i] = from_(s.inst.data.rd);
        trace[cpu_cols::COL_OP1_VALUE][i] =
            from_(s.state.get_register_value(usize::from(s.inst.data.rs1)));
        trace[cpu_cols::COL_OP2_VALUE][i] =
            from_(s.state.get_register_value(usize::from(s.inst.data.rs2)));
        trace[cpu_cols::COL_DST_VALUE][i] =
            from_(s.state.get_register_value(usize::from(s.inst.data.rd)));
        trace[cpu_cols::COL_S_HALT][i] = from_(s.state.has_halted());
        for j in 0..32_usize {
            trace[cpu_cols::COL_START_REG + j][i] = from_(s.state.get_register_value(j));
        }

        match s.inst.op {
            Op::ADD => trace[cpu_cols::COL_S_ADD][i] = F::ONE,
            Op::ADDI => {
                trace[cpu_cols::COL_S_ADD][i] = F::ONE;
                // override value of OP2, as its immediate operand
                trace[cpu_cols::COL_OP2_VALUE][i] = from_(s.inst.data.imm);
            }
            Op::BEQ => trace[cpu_cols::COL_S_BEQ][i] = F::ONE,
            _ => {}
        }
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    let trace = pad_trace(trace);

    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cpu_cols::NUM_CPU_COLS,
            v.len()
        )
    })
}
