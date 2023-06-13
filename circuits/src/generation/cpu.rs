use log::trace as log_trace;
use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::utils::{from_, pad_trace};

#[allow(clippy::missing_panics_doc)]
pub fn generate_cpu_trace<F: RichField>(step_rows: &[Row]) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    // NOTE: Frist row of steps is just initial state without any instruction.
    // All registers value in columns COL_START_REG to COL_START_REG + 31
    // have register values at given clock before executing instruction.
    // All other columns in trace has updated values at given clock after executing
    // current instuction. We do this to make cpu constraint `only_rd_changes()`
    // work correctly.
    let trace_len = step_rows.len() - 1;
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; trace_len]; cpu_cols::NUM_CPU_COLS];
    for i in 1..step_rows.len() {
        let s = &step_rows[i];
        trace[cpu_cols::COL_CLK][i - 1] = from_(s.state.clk);
        trace[cpu_cols::COL_PC][i - 1] = from_(s.state.get_pc());

        trace[cpu_cols::COL_RS1][i - 1] = from_(s.inst.data.rs1);
        trace[cpu_cols::COL_RS2][i - 1] = from_(s.inst.data.rs2);
        trace[cpu_cols::COL_RD][i - 1] = from_(s.inst.data.rd);
        trace[cpu_cols::COL_OP1_VALUE][i - 1] =
            from_(s.state.get_register_value(usize::from(s.inst.data.rs1)));
        trace[cpu_cols::COL_OP2_VALUE][i - 1] =
            from_(s.state.get_register_value(usize::from(s.inst.data.rs2)));
        trace[cpu_cols::COL_DST_VALUE][i - 1] =
            from_(s.state.get_register_value(usize::from(s.inst.data.rd)));
        trace[cpu_cols::COL_IMM_VALUE][i - 1] = from_(s.inst.data.imm);
        trace[cpu_cols::COL_S_HALT][i - 1] = from_(s.state.has_halted());
        for j in 0..32_usize {
            trace[cpu_cols::COL_START_REG + j][i - 1] =
                from_(step_rows[i - 1].state.get_register_value(j));
        }

        match s.inst.op {
            Op::ADD => trace[cpu_cols::COL_S_ADD][i - 1] = F::ONE,
            Op::BEQ => trace[cpu_cols::COL_S_BEQ][i - 1] = F::ONE,
            _ => {}
        }
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    let trace = pad_trace(trace, Some(cpu_cols::COL_CLK));

    log_trace!("trace {:?}", trace);
    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cpu_cols::NUM_CPU_COLS,
            v.len()
        )
    })
}
