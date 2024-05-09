use expr::{Evaluator, ExprBuilder};
use itertools::Itertools;
use log::debug;
use mozak_runner::instruction::{Instruction, Op};
use mozak_runner::state::{Aux, State, StorageDeviceEntry, StorageDeviceOpcode};
use mozak_runner::vm::{ExecutionRecord, Row};
use mozak_sdk::core::ecall;
use mozak_sdk::core::reg_abi::REG_A0;
use plonky2::hash::hash_types::RichField;

use super::MIN_TRACE_LENGTH;
use crate::bitshift::columns::Bitshift;
use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::CpuState;
use crate::cpu_skeleton::columns::CpuSkeleton;
use crate::expr::PureEvaluator;
use crate::program::columns::ProgramRom;
use crate::program_multiplicities::columns::ProgramMult;
use crate::utils::{from_u32, sign_extend};
use crate::xor::columns::XorView;

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<CpuState<F>>) -> Vec<CpuState<F>> {
    let len = trace.len().next_power_of_two().max(MIN_TRACE_LENGTH);
    let padding = CpuState {
        product_high_limb_inv_helper: F::from_canonical_u32(u32::MAX).inverse(),
        quotient_value: F::from_canonical_u32(u32::MAX),
        ..Default::default()
    };

    trace.resize(len, padding);
    trace
}

#[must_use]
pub fn generate_program_mult_trace<F: RichField>(
    skeleton: &[CpuSkeleton<F>],
    program_rom: &[ProgramRom<F>],
) -> Vec<ProgramMult<F>> {
    let mut counts = skeleton
        .iter()
        .filter(|row| row.is_running.is_nonzero())
        .map(|row| row.pc)
        .counts();
    program_rom
        .iter()
        .map(|&inst| ProgramMult {
            // We use `remove` instead of a plain `get` to deal with duplicates (from padding) in
            // the ROM.
            mult_in_cpu: F::from_canonical_usize(counts.remove(&inst.pc).unwrap_or_default()),
            rom_row: inst,
        })
        .collect()
}

/// Converting each row of the `record` to a row represented by [`CpuState`]
pub fn generate_cpu_trace<F: RichField>(record: &ExecutionRecord<F>) -> Vec<CpuState<F>> {
    debug!("Starting CPU Trace Generation");
    let mut trace: Vec<CpuState<F>> = vec![];

    let default_io_entry = StorageDeviceEntry::default();
    for Row {
        state,
        instruction,
        aux,
    } in &record.executed
    {
        let inst = instruction;
        let io = aux
            .storage_device_entry
            .as_ref()
            .unwrap_or(&default_io_entry);
        // Skip instruction handled by their own tables.
        // TODO: refactor, so we don't repeat logic.
        {
            if let Op::ADD | Op::SW | Op::LW = inst.op {
                continue;
            }

            let op1_value = state.get_register_value(inst.args.rs1);
            let op2_value = state.get_register_value(inst.args.rs2);
            if op1_value < op2_value && Op::BLTU == inst.op {
                continue;
            }
        }
        let mut row = CpuState {
            clk: F::from_noncanonical_u64(state.clk),
            new_pc: F::from_canonical_u32(aux.new_pc),
            inst: cpu_cols::Instruction::from((state.get_pc(), *inst)).map(from_u32),
            op1_value: from_u32(aux.op1),
            op2_value_raw: from_u32(aux.op2_raw),
            op2_value: from_u32(aux.op2),
            // This seems reasonable-ish, but it's also suspicious?
            // It seems too simple.
            op2_value_overflowing: from_u32::<F>(state.get_register_value(inst.args.rs2))
                + from_u32(inst.args.imm),
            // NOTE: Updated value of DST register is next step.
            dst_value: from_u32(aux.dst_val),
            // is_running: F::from_bool(!state.halted),
            // Valid defaults for the powers-of-two gadget.
            // To be overridden by users of the gadget.
            // TODO(Matthias): find a way to make either compiler or runtime complain
            // if we have two (conflicting) users in the same row.
            bitshift: Bitshift::from(0).map(F::from_canonical_u32),
            xor: generate_xor_row(inst, state),
            mem_addr: F::from_canonical_u32(aux.mem.unwrap_or_default().addr),
            mem_value_raw: from_u32(aux.mem.unwrap_or_default().raw_value),
            is_poseidon2: F::from_bool(aux.poseidon2.is_some()),
            io_addr: F::from_canonical_u32(io.addr),
            io_size: F::from_canonical_usize(io.data.len()),
            is_private_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StorePrivate)
            )),
            is_public_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StorePublic)
            )),
            is_call_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StoreCallTape)
            )),
            is_event_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StoreEventTape)
            )),
            is_events_commitment_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StoreEventsCommitmentTape)
            )),
            is_cast_list_commitment_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StoreCastListCommitmentTape)
            )),
            is_self_prog_id_tape: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, StorageDeviceOpcode::StoreSelfProgIdTape)
            )),
            is_halt: F::from_bool(matches!(
                (inst.op, state.registers[usize::from(REG_A0)]),
                (Op::ECALL, ecall::HALT)
            )),
            ..CpuState::default()
        };

        generate_shift_row(&mut row, aux);
        generate_mul_row(&mut row, aux);
        generate_div_row(&mut row, inst, aux);
        operands_sign_handling(&mut row, aux);
        memory_sign_handling(&mut row, inst, aux);
        generate_conditional_branch_row(&mut row);
        trace.push(row);
    }

    dbg!(trace.len());
    log::trace!("trace {:?}", trace);

    pad_trace(trace)
}

/// This is a wrapper to make the Expr mechanics work directly with a Field.
///
/// TODO(Matthias): Make this more generally useful.
fn signed_diff<F: RichField>(row: &CpuState<F>) -> F {
    let expr_builder = ExprBuilder::default();
    let row = row.map(|x| expr_builder.lit(x));
    PureEvaluator(F::from_noncanonical_i64).eval(row.signed_diff())
}

fn generate_conditional_branch_row<F: RichField>(row: &mut CpuState<F>) {
    let signed_diff = signed_diff(row);
    row.cmp_diff_inv = signed_diff.try_inverse().unwrap_or_default();
    row.normalised_diff = F::from_bool(signed_diff.is_nonzero());
}

/// Generates a bitshift row on a shift operation. This is used in the bitshift
/// lookup table.
fn generate_shift_row<F: RichField>(row: &mut CpuState<F>, aux: &Aux<F>) {
    let shift_power = aux.op2;
    let shift_amount = if shift_power == 0 {
        0
    } else {
        31_u32 - shift_power.leading_zeros()
    };
    row.bitshift = Bitshift {
        amount: shift_amount,
        multiplier: shift_power,
    }
    .map(from_u32);
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_lossless)]
fn compute_full_range(is_signed: bool, value: u32) -> i64 {
    if is_signed {
        value as i32 as i64
    } else {
        value as i64
    }
}

#[allow(clippy::cast_possible_truncation)]
fn generate_mul_row<F: RichField>(row: &mut CpuState<F>, aux: &Aux<F>) {
    // Helper function to determine sign and absolute value.
    let compute_sign_and_abs: fn(bool, u32) -> (bool, u32) = |is_signed, value| {
        let full_range = compute_full_range(is_signed, value);
        let is_negative = full_range.is_negative();
        let absolute_value = full_range.unsigned_abs() as u32;
        (is_negative, absolute_value)
    };
    let (is_op2_negative, op2_abs) =
        compute_sign_and_abs(row.inst.is_op2_signed.is_nonzero(), aux.op2);
    let (is_op1_negative, op1_abs) =
        compute_sign_and_abs(row.inst.is_op1_signed.is_nonzero(), aux.op1);

    // Determine product sign and absolute value.
    let mut product_sign = is_op1_negative ^ is_op2_negative;
    let op1_mul_op2_abs = u64::from(op1_abs) * u64::from(op2_abs);

    row.skip_check_product_sign = if op1_mul_op2_abs == 0 {
        product_sign = false;
        F::ONE
    } else {
        F::ZERO
    };

    row.product_sign = if product_sign { F::ONE } else { F::ZERO };
    row.op1_abs = from_u32(op1_abs);
    row.op2_abs = from_u32(op2_abs);

    // Compute the product limbs based on sign.
    let prod = if product_sign {
        u64::MAX - op1_mul_op2_abs + 1
    } else {
        op1_mul_op2_abs
    };

    let low = (prod & 0xffff_ffff) as u32;
    let high = (prod >> 32) as u32;
    row.product_low_limb = from_u32(low);
    row.product_high_limb = from_u32(high);

    // Calculate the product high limb inverse helper.
    let inv_helper_val = if product_sign {
        high
    } else {
        0xffff_ffff - high
    };
    row.product_high_limb_inv_helper = from_u32::<F>(inv_helper_val)
        .try_inverse()
        .unwrap_or_default();
}

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
fn generate_div_row<F: RichField>(row: &mut CpuState<F>, inst: &Instruction, aux: &Aux<F>) {
    let dividend_full_range = compute_full_range(row.inst.is_op1_signed.is_nonzero(), aux.op1);
    let divisor_full_range = compute_full_range(row.inst.is_op2_signed.is_nonzero(), aux.op2);

    if divisor_full_range == 0 {
        row.quotient_value = from_u32(0xFFFF_FFFF);
        row.quotient_sign = if row.inst.is_op2_signed.is_nonzero() {
            F::ONE
        } else {
            F::ZERO
        };
        row.remainder_value = from_u32(aux.op1);
        row.remainder_slack = F::ZERO;
        row.remainder_sign = F::from_bool(dividend_full_range.is_negative());
        row.skip_check_quotient_sign = F::ONE;
    } else {
        let quotient_full_range = if matches!(inst.op, Op::SRA) {
            dividend_full_range.div_euclid(divisor_full_range)
        } else {
            dividend_full_range / divisor_full_range
        };
        row.quotient_value = from_u32(quotient_full_range as u32);
        row.quotient_sign = F::from_bool(quotient_full_range.is_negative());
        row.skip_check_quotient_sign = F::from_bool(quotient_full_range == 0);
        if dividend_full_range == -2 ^ 31 && divisor_full_range == -1 {
            // Special case for dividend == -2^31, divisor == -1:
            // quotient_sign == 1 (quotient = -2^31).
            row.skip_check_quotient_sign = F::ONE;
            row.quotient_sign = F::ONE;
        }
        let remainder = dividend_full_range - quotient_full_range * divisor_full_range;
        let remainder_abs = remainder.unsigned_abs();
        row.remainder_value = from_u32(remainder as u32);
        row.remainder_slack =
            F::from_noncanonical_u64(divisor_full_range.unsigned_abs() - 1 - remainder_abs);
        row.remainder_sign = F::from_bool(remainder.is_negative());
    }
    row.op2_value_inv = from_u32::<F>(aux.op2).try_inverse().unwrap_or_default();
}

fn memory_sign_handling<F: RichField>(row: &mut CpuState<F>, inst: &Instruction, aux: &Aux<F>) {
    // sign extension needs to be from `u8` in case of `LB`
    // sign extension needs to be from `u16` in case of `LH`
    row.dst_sign_bit = F::from_bool(match inst.op {
        Op::LB => aux.dst_val >= 1 << 7,
        Op::LH => aux.dst_val >= 1 << 15,
        _ => false,
    });
}

fn operands_sign_handling<F: RichField>(row: &mut CpuState<F>, aux: &Aux<F>) {
    let op1_full_range = sign_extend(row.inst.is_op1_signed.is_nonzero(), aux.op1);
    let op2_full_range = sign_extend(row.inst.is_op2_signed.is_nonzero(), aux.op2);

    row.op1_sign_bit = F::from_bool(op1_full_range < 0);
    row.op2_sign_bit = F::from_bool(op2_full_range < 0);

    row.less_than = F::from_bool(op1_full_range < op2_full_range);
    let abs_diff = op1_full_range.abs_diff(op2_full_range);
    row.abs_diff = F::from_noncanonical_u64(abs_diff);
}

fn generate_xor_row<F: RichField>(inst: &Instruction, state: &State<F>) -> XorView<F> {
    let a = match inst.op {
        Op::AND | Op::OR | Op::XOR | Op::SB | Op::SH => state.get_register_value(inst.args.rs1),
        Op::SRL | Op::SLL | Op::SRA => 0b1_1111,
        _ => 0,
    };
    let b = match inst.op {
        Op::AND | Op::OR | Op::XOR | Op::SRL | Op::SLL | Op::SRA => state
            .get_register_value(inst.args.rs2)
            .wrapping_add(inst.args.imm),
        Op::SB => 0x0000_00FF,
        Op::SH => 0x0000_FFFF,
        _ => 0,
    };
    XorView { a, b, out: a ^ b }.map(from_u32)
}
