use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::state::State;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::MAP;
use crate::utils::{from_u32, pad_trace};

#[allow(clippy::missing_panics_doc)]
pub fn generate_cpu_trace<F: RichField>(step_rows: &[Row]) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; step_rows.len()]; cpu_cols::NUM_CPU_COLS];

    for (i, Row { state, aux }) in step_rows.iter().enumerate() {
        trace[MAP.clk][i] = F::from_noncanonical_u64(state.clk);
        trace[MAP.inst.pc][i] = from_u32(state.get_pc());

        let inst = state.current_instruction();

        trace[MAP.inst.rs1_select[inst.args.rs1 as usize]][i] = F::ONE;
        trace[MAP.inst.rs2_select[inst.args.rs2 as usize]][i] = F::ONE;
        trace[MAP.inst.rd_select[inst.args.rd as usize]][i] = F::ONE;
        trace[MAP.op1_value][i] = from_u32(state.get_register_value(inst.args.rs1));
        // OP2_VALUE is the sum of the value of the second operand register and the
        // immediate value.
        trace[MAP.op2_value][i] = from_u32(
            state
                .get_register_value(inst.args.rs2)
                .wrapping_add(inst.args.imm),
        );
        // NOTE: Updated value of DST register is next step.
        trace[MAP.dst_value][i] = from_u32(aux.dst_val);
        trace[MAP.inst.imm_value][i] = from_u32(inst.args.imm);
        trace[MAP.inst.branch_target][i] = from_u32(inst.args.branch_target);
        trace[MAP.halt][i] = from_u32(u32::from(aux.will_halt));
        for j in 0..32 {
            trace[MAP.regs[j as usize]][i] = from_u32(state.get_register_value(j));
        }

        // Valid defaults for the powers-of-two gadget.
        // To be overridden by users of the gadget.
        // TODO(Matthias): find a way to make either compiler or runtime complain
        // if we have two (conflicting) users in the same row.
        trace[MAP.powers_of_2_in][i] = F::ZERO;
        trace[MAP.powers_of_2_out][i] = F::ONE;

        generate_mul_row(&mut trace, &inst, state, i);
        generate_divu_row(&mut trace, &inst, state, i);
        generate_slt_row(&mut trace, &inst, state, i);
        generate_conditional_branch_row(&mut trace, i);
        generate_bitwise_row(&mut trace, &inst, state, i);

        match inst.op {
            Op::ADD => trace[MAP.inst.ops.add][i] = F::ONE,
            Op::SLL => trace[MAP.inst.ops.sll][i] = F::ONE,
            Op::SLT => trace[MAP.inst.ops.slt][i] = F::ONE,
            Op::SLTU => trace[MAP.inst.ops.sltu][i] = F::ONE,
            Op::SRL => trace[MAP.inst.ops.srl][i] = F::ONE,
            Op::SUB => trace[MAP.inst.ops.sub][i] = F::ONE,
            Op::DIVU => trace[MAP.inst.ops.divu][i] = F::ONE,
            Op::REMU => trace[MAP.inst.ops.remu][i] = F::ONE,
            Op::MUL => trace[MAP.inst.ops.mul][i] = F::ONE,
            Op::MULHU => trace[MAP.inst.ops.mulhu][i] = F::ONE,
            Op::JALR => trace[MAP.inst.ops.jalr][i] = F::ONE,
            Op::BEQ => trace[MAP.inst.ops.beq][i] = F::ONE,
            Op::BNE => trace[MAP.inst.ops.bne][i] = F::ONE,
            Op::ECALL => trace[MAP.inst.ops.ecall][i] = F::ONE,
            Op::XOR => trace[MAP.inst.ops.xor][i] = F::ONE,
            Op::OR => trace[MAP.inst.ops.or][i] = F::ONE,
            Op::AND => trace[MAP.inst.ops.and][i] = F::ONE,
            #[tarpaulin::skip]
            _ => {}
        }
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    let trace = pad_trace(trace, Some(MAP.clk));

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
fn generate_conditional_branch_row<F: RichField>(trace: &mut [Vec<F>], row_idx: usize) {
    let diff = trace[MAP.op1_value][row_idx] - trace[MAP.op2_value][row_idx];
    let diff_inv = diff.try_inverse().unwrap_or_default();

    trace[MAP.cmp_diff_inv][row_idx] = diff_inv;
    trace[MAP.branch_equal][row_idx] = F::ONE - diff * diff_inv;
}

#[allow(clippy::cast_possible_wrap)]
fn generate_mul_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    row_idx: usize,
) {
    if !matches!(inst.op, Op::MUL | Op::MULHU | Op::SLL) {
        return;
    }
    let op1 = state.get_register_value(inst.args.rs1);
    let op2 = state
        .get_register_value(inst.args.rs2)
        .wrapping_add(inst.args.imm);
    let multiplier = if let Op::SLL = inst.op {
        let shift_amount = op2 & 0x1F;
        let shift_power = 1_u32 << shift_amount;
        trace[MAP.powers_of_2_in][row_idx] = from_u32(shift_amount);
        trace[MAP.powers_of_2_out][row_idx] = from_u32(shift_power);
        shift_power
    } else {
        op2
    };

    trace[MAP.multiplier][row_idx] = from_u32(multiplier);
    let (low, high) = op1.widening_mul(multiplier);
    trace[MAP.product_low_bits][row_idx] = from_u32(low);
    trace[MAP.product_high_bits][row_idx] = from_u32(high);

    // Prove that the high limb is different from `u32::MAX`:
    let high_diff: F = from_u32(u32::MAX - high);
    trace[MAP.product_high_diff_inv][row_idx] = high_diff.try_inverse().unwrap_or_default();
}

#[allow(clippy::cast_possible_wrap)]
fn generate_divu_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    row_idx: usize,
) {
    let dividend = state.get_register_value(inst.args.rs1);
    let op2 = state
        .get_register_value(inst.args.rs2)
        .wrapping_add(inst.args.imm);

    let divisor = if let Op::SRL = inst.op {
        let shift_amount = op2 & 0x1F;
        let shift_power = 1_u32 << shift_amount;
        trace[MAP.powers_of_2_in][row_idx] = from_u32(shift_amount);
        trace[MAP.powers_of_2_out][row_idx] = from_u32(shift_power);
        shift_power
    } else {
        op2
    };

    trace[MAP.divisor][row_idx] = from_u32(divisor);

    if let 0 = divisor {
        trace[MAP.quotient][row_idx] = from_u32(u32::MAX);
        trace[MAP.remainder][row_idx] = from_u32(dividend);
        trace[MAP.remainder_slack][row_idx] = from_u32(0_u32);
    } else {
        trace[MAP.quotient][row_idx] = from_u32(dividend / divisor);
        trace[MAP.remainder][row_idx] = from_u32(dividend % divisor);
        trace[MAP.remainder_slack][row_idx] = from_u32(divisor - dividend % divisor - 1);
    }
    trace[MAP.divisor_inv][row_idx] = from_u32::<F>(divisor).try_inverse().unwrap_or_default();
}

#[allow(clippy::cast_possible_wrap)]
fn generate_slt_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    row_idx: usize,
) {
    let is_signed = inst.op == Op::SLT;
    let op1 = state.get_register_value(inst.args.rs1);
    let op2 = state.get_register_value(inst.args.rs2) + inst.args.imm;
    let sign1: u32 = (is_signed && (op1 as i32) < 0).into();
    let sign2: u32 = (is_signed && (op2 as i32) < 0).into();
    trace[MAP.op1_sign][row_idx] = from_u32(sign1);
    trace[MAP.op2_sign][row_idx] = from_u32(sign2);

    let sign_adjust = if is_signed { 1 << 31 } else { 0 };
    let op1_fixed = op1.wrapping_add(sign_adjust);
    let op2_fixed = op2.wrapping_add(sign_adjust);
    trace[MAP.op1_val_fixed][row_idx] = from_u32(op1_fixed);
    trace[MAP.op2_val_fixed][row_idx] = from_u32(op2_fixed);
    trace[MAP.less_than][row_idx] = from_u32(u32::from(op1_fixed < op2_fixed));

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
    trace[MAP.cmp_abs_diff][row_idx] = from_u32(abs_diff_fixed);
}

fn generate_bitwise_row<F: RichField>(
    trace: &mut [Vec<F>],
    inst: &Instruction,
    state: &State,
    i: usize,
) {
    let op1 = match inst.op {
        Op::AND | Op::OR | Op::XOR => state.get_register_value(inst.args.rs1),
        Op::SRL | Op::SLL => 0x1F,
        _ => 0,
    };
    let op2 = state
        .get_register_value(inst.args.rs2)
        .wrapping_add(inst.args.imm);
    trace[MAP.xor_a][i] = from_u32(op1);
    trace[MAP.xor_b][i] = from_u32(op2);
    trace[MAP.xor_out][i] = from_u32(op1 ^ op2);
}
