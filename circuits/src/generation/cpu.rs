use itertools::{chain, Itertools};
use mozak_vm::elf::Program;
use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::state::State;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::Bitshift;
use crate::bitwise::columns::XorView;
use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::{CpuColumnsExtended, CpuColumnsView};
use crate::program::columns::{InstColumnsView, ProgramColumnsView};
use crate::stark::utils::transpose_trace;
use crate::utils::from_u32;

/// Pad the trace to a power of 2.
///
/// # Panics
/// There's an assert that makes sure all columns passed in have the same
/// length.
#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<CpuColumnsView<F>>) -> Vec<CpuColumnsView<F>> {
    trace.resize(trace.len().next_power_of_two(), *trace.last().unwrap());
    trace
}

#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn generate_cpu_trace_extended<F: RichField>(
    cpu_trace: Vec<CpuColumnsView<F>>,
) -> CpuColumnsExtended<Vec<F>> {
    let permuted = generate_permuted_inst_trace(&cpu_trace);
    (chain!(transpose_trace(cpu_trace), transpose_trace(permuted))).collect()
}

pub fn generate_cpu_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row],
) -> Vec<CpuColumnsView<F>> {
    // let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; step_rows.len()];
    // cpu_cols::NUM_CPU_COLS];
    let mut trace: Vec<CpuColumnsView<F>> = vec![];

    for Row { state, aux } in step_rows {
        let inst = state.current_instruction(program);
        let mut row = CpuColumnsView {
            clk: F::from_noncanonical_u64(state.clk),
            inst: cpu_cols::InstructionView::from((state.get_pc(), inst)).map(from_u32),
            op1_value: from_u32(state.get_register_value(inst.args.rs1)),
            // OP2_VALUE is the sum of the value of the second operand register and the
            // immediate value.
            op2_value: from_u32(
                state
                    .get_register_value(inst.args.rs2)
                    .wrapping_add(inst.args.imm),
            ),
            // NOTE: Updated value of DST register is next step.
            dst_value: from_u32(aux.dst_val),
            halt: from_u32(u32::from(aux.will_halt)),
            // Valid defaults for the powers-of-two gadget.
            // To be overridden by users of the gadget.
            // TODO(Matthias): find a way to make either compiler or runtime complain
            // if we have two (conflicting) users in the same row.
            bitshift: Bitshift::from(0).map(F::from_canonical_u64),
            xor: generate_bitwise_row(&inst, state),

            ..CpuColumnsView::default()
        };

        for j in 0..32 {
            row.regs[j as usize] = from_u32(state.get_register_value(j));
        }

        generate_mul_row(&mut row, &inst, state);
        generate_divu_row(&mut row, &inst, state);
        generate_slt_row(&mut row, &inst, state);
        generate_conditional_branch_row(&mut row);
        trace.push(row);
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    let trace = pad_trace(trace);

    log::trace!("trace {:?}", trace);
    trace
}

fn generate_conditional_branch_row<F: RichField>(row: &mut CpuColumnsView<F>) {
    let diff = row.op1_value - row.op2_value;
    let diff_inv = diff.try_inverse().unwrap_or_default();

    row.cmp_diff_inv = diff_inv;
    row.branch_equal = F::ONE - diff * diff_inv;
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::similar_names)]
fn generate_mul_row<F: RichField>(row: &mut CpuColumnsView<F>, inst: &Instruction, state: &State) {
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
        row.bitshift = Bitshift {
            amount: shift_amount,
            multiplier: shift_power,
        }
        .map(from_u32);
        shift_power
    } else {
        op2
    };

    row.multiplier = from_u32(multiplier);
    let (low, high) = op1.widening_mul(multiplier);
    row.product_low_bits = from_u32(low);
    row.product_high_bits = from_u32(high);

    // Prove that the high limb is different from `u32::MAX`:
    let high_diff: F = from_u32(u32::MAX - high);
    row.product_high_diff_inv = high_diff.try_inverse().unwrap_or_default();
}

#[allow(clippy::cast_possible_wrap)]
fn generate_divu_row<F: RichField>(row: &mut CpuColumnsView<F>, inst: &Instruction, state: &State) {
    let dividend = state.get_register_value(inst.args.rs1);
    let op2 = state
        .get_register_value(inst.args.rs2)
        .wrapping_add(inst.args.imm);

    let divisor = if let Op::SRL = inst.op {
        let shift_amount = op2 & 0x1F;
        let shift_power = 1_u32 << shift_amount;
        row.bitshift = Bitshift {
            amount: shift_amount,
            multiplier: shift_power,
        }
        .map(from_u32);
        shift_power
    } else {
        op2
    };

    row.divisor = from_u32(divisor);

    if let 0 = divisor {
        row.quotient = from_u32(u32::MAX);
        row.remainder = from_u32(dividend);
        row.remainder_slack = from_u32(0_u32);
    } else {
        row.quotient = from_u32(dividend / divisor);
        row.remainder = from_u32(dividend % divisor);
        row.remainder_slack = from_u32(divisor - dividend % divisor - 1);
    }
    row.divisor_inv = from_u32::<F>(divisor).try_inverse().unwrap_or_default();
}

#[allow(clippy::cast_possible_wrap)]
fn generate_slt_row<F: RichField>(row: &mut CpuColumnsView<F>, inst: &Instruction, state: &State) {
    let is_signed = inst.op == Op::SLT;
    let op1 = state.get_register_value(inst.args.rs1);
    let op2 = state.get_register_value(inst.args.rs2) + inst.args.imm;
    let sign1: u32 = (is_signed && (op1 as i32) < 0).into();
    let sign2: u32 = (is_signed && (op2 as i32) < 0).into();
    row.op1_sign = from_u32(sign1);
    row.op2_sign = from_u32(sign2);

    let sign_adjust = if is_signed { 1 << 31 } else { 0 };
    let op1_fixed = op1.wrapping_add(sign_adjust);
    let op2_fixed = op2.wrapping_add(sign_adjust);
    row.op1_val_fixed = from_u32(op1_fixed);
    row.op2_val_fixed = from_u32(op2_fixed);
    row.less_than = from_u32(u32::from(op1_fixed < op2_fixed));

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
    row.cmp_abs_diff = from_u32(abs_diff_fixed);
}

fn generate_bitwise_row<F: RichField>(inst: &Instruction, state: &State) -> XorView<F> {
    let a = match inst.op {
        Op::AND | Op::OR | Op::XOR => state.get_register_value(inst.args.rs1),
        Op::SRL | Op::SLL => 0x1F,
        _ => 0,
    };
    let b = state
        .get_register_value(inst.args.rs2)
        .wrapping_add(inst.args.imm);
    XorView { a, b, out: a ^ b }.map(from_u32)
}

#[must_use]
pub fn generate_permuted_inst_trace<F: RichField>(
    trace: &[CpuColumnsView<F>],
) -> Vec<ProgramColumnsView<F>> {
    trace
        .iter()
        .map(|row| row.inst)
        .sorted_by_key(|inst| inst.pc.to_noncanonical_u64())
        .scan(None, |previous_pc, inst| {
            Some(ProgramColumnsView {
                filter: F::from_bool(Some(inst.pc) == previous_pc.replace(inst.pc)),
                inst: InstColumnsView::from(inst),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use plonky2::field::types::Field;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::cpu::columns::NUM_CPU_COLS;

    #[test]
    fn test_generate_permuted_inst_trace() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let _trace: Vec<Vec<F>> = vec![vec![F::ZERO; 4]; NUM_CPU_COLS];

        // trace[MAP.inst.pc] = vec![from_u32(3), from_u32(1), from_u32(2),
        // from_u32(1)]; trace[MAP.opcode] = vec![from_u32(2),
        // from_u32(3), from_u32(1), from_u32(4)]; trace[MAP.rs1] =
        // vec![from_u32(1), from_u32(2), from_u32(3), from_u32(4)];
        // trace[MAP.rs2] = vec![from_u32(2), from_u32(1), from_u32(3),
        // from_u32(4)]; trace[MAP.rd] = vec![from_u32(3), from_u32(1),
        // from_u32(2), from_u32(4)]; trace[MAP.imm_value] =
        // vec![from_u32(1), from_u32(3), from_u32(2), from_u32(4)];

        // let permuted_trace = generate_permuted_inst_trace(trace);

        // assert_eq!(permuted_trace[MAP.permuted_pc], [
        //     from_u32(1),
        //     from_u32(1),
        //     from_u32(2),
        //     from_u32(3)
        // ]);
        // assert_eq!(permuted_trace[MAP.permuted_opcode], [
        //     from_u32(3),
        //     from_u32(4),
        //     from_u32(1),
        //     from_u32(2)
        // ]);
        // assert_eq!(permuted_trace[MAP.permuted_rs1], [
        //     from_u32(2),
        //     from_u32(4),
        //     from_u32(3),
        //     from_u32(1)
        // ]);
        // assert_eq!(permuted_trace[MAP.permuted_rs2], [
        //     from_u32(1),
        //     from_u32(4),
        //     from_u32(3),
        //     from_u32(2)
        // ]);
        // assert_eq!(permuted_trace[MAP.permuted_rd], [
        //     from_u32(1),
        //     from_u32(4),
        //     from_u32(2),
        //     from_u32(3)
        // ]);
        // assert_eq!(permuted_trace[MAP.permuted_imm], [
        //     from_u32(3),
        //     from_u32(4),
        //     from_u32(2),
        //     from_u32(1)
        // ]);
        // assert_eq!(permuted_trace[MAP.duplicate_inst_filter], [
        //     from_u32(1),
        //     from_u32(0),
        //     from_u32(1),
        //     from_u32(1)
        // ]);
    }
}
