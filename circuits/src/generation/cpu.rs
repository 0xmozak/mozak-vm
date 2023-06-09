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

    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; cpu_cols::NUM_CPU_COLS];
    for (i, s) in step_rows.iter().enumerate() {
        trace[cpu_cols::COL_CLK][i] = F::from_canonical_usize(s.state.clk);
        trace[cpu_cols::COL_PC][i] = F::from_canonical_u32(s.state.get_pc());

        trace[cpu_cols::COL_RS1][i] = F::from_canonical_u8(s.inst.data.rs1);
        trace[cpu_cols::COL_RS2][i] = F::from_canonical_u8(s.inst.data.rs2);
        trace[cpu_cols::COL_RD][i] = F::from_canonical_u8(s.inst.data.rd);
        trace[cpu_cols::COL_OP1_VALUE][i] =
            F::from_canonical_u32(s.state.get_register_value(usize::from(s.inst.data.rs1)));
        // TODO(Vivek): Soon we support immediate values as opd2 in some instructions.
        // So below line will change accordingly.
        trace[cpu_cols::COL_OP2_VALUE][i] =
            F::from_canonical_u32(s.state.get_register_value(usize::from(s.inst.data.rs2)));
        trace[cpu_cols::COL_DST_VALUE][i] =
            F::from_canonical_u32(s.state.get_register_value(usize::from(s.inst.data.rd)));
        trace[cpu_cols::COL_S_HALT][i] = F::from_canonical_u8(s.state.has_halted().into());
        for j in 0..32_usize {
            trace[cpu_cols::COL_START_REG + j][i] =
                F::from_canonical_u32(s.state.get_register_value(j));
        }

        match s.inst.op {
            Op::ADD => trace[cpu_cols::COL_S_ADD][i] = F::from_canonical_u8(1),
            Op::BEQ => trace[cpu_cols::COL_S_BEQ][i] = F::from_canonical_u8(1),
            _ => {}
        }
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    if trace_len != ext_trace_len {
        trace[cpu_cols::COL_CLK..cpu_cols::NUM_CPU_COLS]
            .iter_mut()
            .for_each(|row| {
                let last = row[trace_len - 1];
                row[trace_len..].fill(last);
            });
    }

    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cpu_cols::NUM_CPU_COLS,
            v.len()
        )
    })
}
