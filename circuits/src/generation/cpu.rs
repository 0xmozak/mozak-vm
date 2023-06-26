use mozak_vm::instruction::Op;
use mozak_vm::state::Aux;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::utils::{from_, pad_trace};

#[allow(clippy::missing_panics_doc)]
#[allow(clippy::cast_possible_wrap)]
pub fn generate_cpu_trace<F: RichField>(step_rows: &[Row]) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; step_rows.len()]; cpu_cols::NUM_CPU_COLS];

    for (
        i,
        Row {
            state: s,
            aux: Aux {
                dst_val, will_halt, ..
            },
        },
    ) in step_rows.iter().enumerate()
    {
        trace[cpu_cols::COL_CLK][i] = from_(s.clk);
        trace[cpu_cols::COL_PC][i] = from_(s.get_pc());

        let inst = s.current_instruction();

        trace[cpu_cols::COL_RS1][i] = from_(inst.args.rs1);
        trace[cpu_cols::COL_RS2][i] = from_(inst.args.rs2);
        trace[cpu_cols::COL_RD][i] = from_(inst.args.rd);
        trace[cpu_cols::COL_OP1_VALUE][i] = from_(s.get_register_value(inst.args.rs1));
        trace[cpu_cols::COL_OP2_VALUE][i] = from_(s.get_register_value(inst.args.rs2));
        // NOTE: Updated value of DST register is next step.
        trace[cpu_cols::COL_DST_VALUE][i] = from_(*dst_val);
        trace[cpu_cols::COL_IMM_VALUE][i] = from_(inst.args.imm);
        trace[cpu_cols::COL_S_HALT][i] = from_(u32::from(*will_halt));
        for j in 0..32 {
            trace[cpu_cols::COL_START_REG + j as usize][i] = from_(s.get_register_value(j));
        }

        {
            // CMP
            let is_signed = inst.op == Op::SLT;
            let op1 = s.get_register_value(inst.args.rs1);
            let op2 = s.get_register_value(inst.args.rs2) + inst.args.imm;
            let sign1: u32 = (is_signed && (op1 as i32) < 0).into();
            let sign2: u32 = (is_signed && (op2 as i32) < 0).into();
            trace[cpu_cols::COL_S_SLT_SIGN1][i] = from_(sign1);
            trace[cpu_cols::COL_S_SLT_SIGN2][i] = from_(sign2);

            let sign_adjust = if is_signed { 1 << 31 } else { 0 };
            let op1_fixed = op1.wrapping_add(sign_adjust);
            let op2_fixed = op2.wrapping_add(sign_adjust);
            trace[cpu_cols::COL_S_SLT_OP1_VAL_FIXED][i] = from_(op1_fixed);
            trace[cpu_cols::COL_S_SLT_OP2_VAL_FIXED][i] = from_(op2_fixed);

            let abs_diff = if is_signed {
                (op1 as i32).abs_diff(op2 as i32)
            } else {
                op1.abs_diff(op2)
            };
            {
                if is_signed {
                    assert_eq!(
                        op1 as i32 as i64 - op2 as i32 as i64,
                        op1_fixed as i64 - op2_fixed as i64
                    );
                } else {
                    assert_eq!(
                        op1 as i64 - (op2 as i64),
                        op1_fixed as i64 - (op2_fixed as i64),
                        "{op1} - {op2} != {op1_fixed} - {op2_fixed}"
                    );
                }
            }
            let abs_diff_fixed: u32 = op1_fixed.abs_diff(op2_fixed);
            assert_eq!(abs_diff, abs_diff_fixed);
            trace[cpu_cols::COL_CMP_ABS_DIFF][i] = from_(abs_diff_fixed);

            {
                // let diff = from_::<_, F>(op1) - from_(op2);
                let diff = trace[cpu_cols::COL_OP1_VALUE][i]
                    - trace[cpu_cols::COL_OP2_VALUE][i]
                    - trace[cpu_cols::COL_IMM_VALUE][i];
                let diff_inv = diff.try_inverse().unwrap_or_default();
                trace[cpu_cols::COL_CMP_DIFF_INV][i] = diff_inv;
                let one: F = diff * diff_inv;
                assert_eq!(one, if op1 == op2 { F::ZERO } else { F::ONE });
            }
        }

        match inst.op {
            Op::ADD => trace[cpu_cols::COL_S_ADD][i] = F::ONE,
            Op::BEQ => trace[cpu_cols::COL_S_BEQ][i] = F::ONE,
            Op::SLT => trace[cpu_cols::COL_S_SLT][i] = F::ONE,
            Op::SLTU => trace[cpu_cols::COL_S_SLTU][i] = F::ONE,
            Op::SUB => trace[cpu_cols::COL_S_SUB][i] = F::ONE,
            Op::ECALL => trace[cpu_cols::COL_S_ECALL][i] = F::ONE,
            #[tarpaulin::skip]
            _ => {}
        }
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    let trace = pad_trace(trace, Some(cpu_cols::COL_CLK));

    log::trace!("trace {:?}", trace);
    #[tarpaulin::skip]
    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cpu_cols::NUM_CPU_COLS,
            v.len()
        )
    })
}
