use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::state::State;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::utils::{from_, pad_trace};

#[allow(clippy::missing_panics_doc)]
pub fn generate_cpu_trace<F: RichField>(step_rows: &[Row]) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; step_rows.len()]; cpu_cols::NUM_CPU_COLS];

    for (i, Row { state, aux }) in step_rows.iter().enumerate() {
        trace[cpu_cols::COL_CLK][i] = from_(state.clk);
        trace[cpu_cols::COL_PC][i] = from_(state.get_pc());

        let inst = state.current_instruction();

        trace[cpu_cols::COL_RS1_SELECT[inst.args.rs1 as usize]][i] = F::ONE;
        trace[cpu_cols::COL_RS2_SELECT[inst.args.rs2 as usize]][i] = F::ONE;
        trace[cpu_cols::COL_RD_SELECT[inst.args.rd as usize]][i] = F::ONE;
        trace[cpu_cols::COL_OP1_VALUE][i] = from_(state.get_register_value(inst.args.rs1));
        trace[cpu_cols::COL_OP2_VALUE][i] = from_(state.get_register_value(inst.args.rs2));
        // NOTE: Updated value of DST register is next step.
        trace[cpu_cols::COL_DST_VALUE][i] = from_(aux.dst_val);
        trace[cpu_cols::COL_IMM_VALUE][i] = from_(inst.args.imm);
        trace[cpu_cols::COL_S_HALT][i] = from_(u32::from(aux.will_halt));
        for j in 0..32 {
            trace[cpu_cols::COL_START_REG + j as usize][i] = from_(state.get_register_value(j));
        }

        generate_divu_row(&mut trace, &inst, state, i);
        generate_slt_row(&mut trace, &inst, state, i);

        generate_bitwise_row(&mut trace, &inst, state, i);

        match inst.op {
            Op::ADD => {
                trace[cpu_cols::COL_S_RC][i] = F::ONE;
                trace[cpu_cols::COL_S_ADD][i] = F::ONE;
            }
            Op::SLT => trace[cpu_cols::COL_S_SLT][i] = F::ONE,
            Op::SLTU => trace[cpu_cols::COL_S_SLTU][i] = F::ONE,
            Op::SRL => trace[cpu_cols::COL_S_SRL][i] = F::ONE,
            Op::SUB => trace[cpu_cols::COL_S_SUB][i] = F::ONE,
            Op::DIVU => trace[cpu_cols::COL_S_DIVU][i] = F::ONE,
            Op::REMU => trace[cpu_cols::COL_S_REMU][i] = F::ONE,
            Op::BEQ => trace[cpu_cols::COL_S_BEQ][i] = F::ONE,
            Op::ECALL => trace[cpu_cols::COL_S_ECALL][i] = F::ONE,
            Op::XOR => trace[cpu_cols::COL_S_XOR][i] = F::ONE,
            Op::OR => trace[cpu_cols::COL_S_OR][i] = F::ONE,
            Op::AND => trace[cpu_cols::COL_S_AND][i] = F::ONE,
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

#[allow(clippy::cast_possible_wrap)]
fn generate_divu_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    row_idx: usize,
) {
    let op1 = state.get_register_value(inst.args.rs1);
    let op2 = state.get_register_value(inst.args.rs2) + inst.args.imm;
    let divisor = if let Op::SRL = inst.op {
        1 << (op2 & 0x1F)
    } else {
        op2
    };
    trace[cpu_cols::DIVISOR][row_idx] = from_(divisor);
    if let 0 = divisor {
        trace[cpu_cols::QUOTIENT][row_idx] = from_(u32::MAX);
        trace[cpu_cols::REMAINDER][row_idx] = from_(op1);
        trace[cpu_cols::REMAINDER_SLACK][row_idx] = from_(0_u32);
    } else {
        trace[cpu_cols::QUOTIENT][row_idx] = from_(op1 / divisor);
        trace[cpu_cols::REMAINDER][row_idx] = from_(op1 % divisor);
        trace[cpu_cols::REMAINDER_SLACK][row_idx] = from_(divisor - op1 % divisor - 1);
    }
    trace[cpu_cols::DIVISOR_INV][row_idx] =
        from_::<_, F>(divisor).try_inverse().unwrap_or_default();
}

#[allow(clippy::cast_possible_wrap)]
fn generate_slt_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    row_idx: usize,
) {
    if inst.op != Op::SLT && inst.op != Op::SLTU {
        return;
    }
    let is_signed = inst.op == Op::SLT;
    let op1 = state.get_register_value(inst.args.rs1);
    let op2 = state.get_register_value(inst.args.rs2) + inst.args.imm;
    let sign1: u32 = (is_signed && (op1 as i32) < 0).into();
    let sign2: u32 = (is_signed && (op2 as i32) < 0).into();
    trace[cpu_cols::COL_S_SLT_SIGN1][row_idx] = from_(sign1);
    trace[cpu_cols::COL_S_SLT_SIGN2][row_idx] = from_(sign2);

    let sign_adjust = if is_signed { 1 << 31 } else { 0 };
    let op1_fixed = op1.wrapping_add(sign_adjust);
    let op2_fixed = op2.wrapping_add(sign_adjust);
    trace[cpu_cols::COL_S_SLT_OP1_VAL_FIXED][row_idx] = from_(op1_fixed);
    trace[cpu_cols::COL_S_SLT_OP2_VAL_FIXED][row_idx] = from_(op2_fixed);
    trace[cpu_cols::COL_LESS_THAN][row_idx] = from_(u32::from(op1_fixed < op2_fixed));

    let abs_diff = if is_signed {
        (op1 as i32).abs_diff(op2 as i32)
    } else {
        op1.abs_diff(op2)
    };
    {
        if is_signed {
            assert_eq!(
                i64::from(op1 as i32) - i64::from(op2 as i32),
                i64::from(op1_fixed) - i64::from(op2_fixed)
            );
        } else {
            assert_eq!(
                i64::from(op1) - i64::from(op2),
                i64::from(op1_fixed) - i64::from(op2_fixed),
                "{op1} - {op2} != {op1_fixed} - {op2_fixed}"
            );
        }
    }
    let abs_diff_fixed: u32 = op1_fixed.abs_diff(op2_fixed);
    assert_eq!(abs_diff, abs_diff_fixed);
    trace[cpu_cols::COL_CMP_ABS_DIFF][row_idx] = from_(abs_diff_fixed);

    {
        let diff = trace[cpu_cols::COL_OP1_VALUE][row_idx]
            - trace[cpu_cols::COL_OP2_VALUE][row_idx]
            - trace[cpu_cols::COL_IMM_VALUE][row_idx];
        let diff_inv = diff.try_inverse().unwrap_or_default();
        trace[cpu_cols::COL_CMP_DIFF_INV][row_idx] = diff_inv;
        let one: F = diff * diff_inv;
        assert_eq!(one, if op1 == op2 { F::ZERO } else { F::ONE });
    }
}

fn generate_bitwise_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    i: usize,
) {
    let op1 = state.get_register_value(inst.args.rs1);
    let op2 = state.get_register_value(inst.args.rs2) + inst.args.imm;
    let (a, b) = match inst.op {
        // x ^ y == x + y - 2 * (x & y)
        Op::AND | Op::XOR => (op1, op2),
        // x | y == !(!x & !y)
        // with !z == u32::MAX - z
        Op::OR => (!op1, !op2),
        #[tarpaulin::skip]
        _ => (0, 0),
    };
    trace[cpu_cols::AND_A][i] = from_(a);
    trace[cpu_cols::AND_B][i] = from_(b);
    trace[cpu_cols::AND_OUT][i] = from_(a & b);
}
