use std::collections::HashSet;

use itertools::{chain, Itertools};
use mozak_runner::instruction::{Instruction, Op};
use mozak_runner::state::{Aux, IoEntry, IoOpcode, State};
use mozak_runner::vm::{ExecutionRecord, Row};
use mozak_sdk::core::ecall;
use mozak_sdk::core::reg_abi::REG_A0;
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::Bitshift;
use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::{CpuColumnsExtended, CpuState};
use crate::generation::MIN_TRACE_LENGTH;
use crate::program::columns::{InstructionRow, ProgramRom};
use crate::stark::utils::transpose_trace;
use crate::utils::{from_u32, pad_trace_with_last_to_len, sign_extend};
use crate::xor::columns::XorView;

#[must_use]
pub fn generate_cpu_trace_extended<F: RichField>(
    cpu_trace: Vec<CpuState<F>>,
    program_rom: &[ProgramRom<F>],
) -> CpuColumnsExtended<Vec<F>> {
    let mut permuted = generate_permuted_inst_trace(&cpu_trace, program_rom);
    let len = cpu_trace
        .len()
        .max(permuted.len())
        .max(MIN_TRACE_LENGTH)
        .next_power_of_two();
    let ori_len = permuted.len();
    permuted = pad_trace_with_last_to_len(permuted, len);
    for entry in permuted.iter_mut().skip(ori_len) {
        entry.filter = F::ZERO;
    }
    let cpu_trace = pad_trace_with_last_to_len(cpu_trace, len);
    chain!(transpose_trace(cpu_trace), transpose_trace(permuted)).collect()
}

/// Converting each row of the `record` to a row represented by [`CpuState`]
pub fn generate_cpu_trace<F: RichField>(record: &ExecutionRecord<F>) -> Vec<CpuState<F>> {
    let mut trace: Vec<CpuState<F>> = vec![];
    let ExecutionRecord {
        executed,
        last_state,
    } = record;
    let last_row = &[Row {
        state: last_state.clone(),
        // `Aux` has auxiliary information about an executed CPU cycle.
        // The last state is the final state after the last execution.  Thus naturally it has no
        // associated auxiliary execution information. We use a dummy aux to make the row
        // generation work, but we could refactor to make this unnecessary.
        ..executed.last().unwrap().clone()
    }];

    let default_io_entry = IoEntry::default();
    for Row {
        state,
        instruction,
        aux,
    } in chain![executed, last_row]
    {
        let inst = *instruction;
        let io = aux.io.as_ref().unwrap_or(&default_io_entry);
        let mut row = CpuState {
            clk: F::from_noncanonical_u64(state.clk),
            inst: cpu_cols::Instruction::from((state.get_pc(), inst)).map(from_u32),
            op1_value: from_u32(aux.op1),
            op2_value: from_u32(aux.op2),
            op2_value_overflowing: from_u32::<F>(state.get_register_value(inst.args.rs2))
                + from_u32(inst.args.imm),
            // NOTE: Updated value of DST register is next step.
            dst_value: from_u32(aux.dst_val),
            is_running: F::from_bool(!state.halted),
            // Valid defaults for the powers-of-two gadget.
            // To be overridden by users of the gadget.
            // TODO(Matthias): find a way to make either compiler or runtime complain
            // if we have two (conflicting) users in the same row.
            bitshift: Bitshift::from(0).map(F::from_canonical_u32),
            xor: generate_xor_row(&inst, state),
            mem_addr: F::from_canonical_u32(aux.mem.unwrap_or_default().addr),
            mem_value_raw: from_u32(aux.mem.unwrap_or_default().raw_value),
            #[cfg(feature = "enable_poseidon_starks")]
            is_poseidon2: F::from_bool(aux.poseidon2.is_some()),
            #[cfg(feature = "enable_poseidon_starks")]
            poseidon2_input_addr: F::from_canonical_u32(
                aux.poseidon2.clone().unwrap_or_default().addr,
            ),
            #[cfg(feature = "enable_poseidon_starks")]
            poseidon2_input_len: F::from_canonical_u32(
                aux.poseidon2.clone().unwrap_or_default().len,
            ),
            io_addr: F::from_canonical_u32(io.addr),
            io_size: F::from_canonical_usize(io.data.len()),
            is_io_store_private: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, IoOpcode::StorePrivate)
            )),
            is_io_store_public: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, IoOpcode::StorePublic)
            )),
            is_io_transcript: F::from_bool(matches!(
                (inst.op, io.op),
                (Op::ECALL, IoOpcode::StoreTranscript)
            )),

            is_halt: F::from_bool(matches!(
                (inst.op, state.registers[usize::from(REG_A0)]),
                (Op::ECALL, ecall::HALT)
            )),
            ..CpuState::default()
        };

        for j in 0..32 {
            row.regs[j as usize] = from_u32(state.get_register_value(j));
        }

        generate_shift_row(&mut row, aux);
        generate_mul_row(&mut row, aux);
        generate_div_row(&mut row, &inst, aux);
        operands_sign_handling(&mut row, aux);
        memory_sign_handling(&mut row, &inst, aux);
        generate_conditional_branch_row(&mut row);
        trace.push(row);
    }

    log::trace!("trace {:?}", trace);
    trace
}

fn generate_conditional_branch_row<F: RichField>(row: &mut CpuState<F>) {
    row.cmp_diff_inv = row.signed_diff().try_inverse().unwrap_or_default();
    row.normalised_diff = F::from_bool(row.signed_diff().is_nonzero());
}

/// Generates a bitshift row on a shift operation. This is used in the bitshift
/// lookup table.
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::similar_names)]
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

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::similar_names)]
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

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_lossless)]
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

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_lossless)]
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

// TODO:  a more elegant approach might be move them to the backend using logUp
// or a similar method.
#[must_use]
pub fn generate_permuted_inst_trace<F: RichField>(
    trace: &[CpuState<F>],
    program_rom: &[ProgramRom<F>],
) -> Vec<ProgramRom<F>> {
    let mut cpu_trace: Vec<_> = trace
        .iter()
        .filter(|row| row.is_running == F::ONE)
        .map(|row| row.inst)
        .sorted_by_key(|inst| inst.pc.to_noncanonical_u64())
        .scan(None, |previous_pc, inst| {
            Some(ProgramRom {
                filter: F::from_bool(Some(inst.pc) != previous_pc.replace(inst.pc)),
                inst: InstructionRow::from(inst),
            })
        })
        .collect();

    let used_pcs: HashSet<F> = cpu_trace.iter().map(|row| row.inst.pc).collect();

    // Filter program_rom to contain only instructions with the pc that are not in
    // used_pcs
    let unused_instructions: Vec<_> = program_rom
        .iter()
        .filter(|row| !used_pcs.contains(&row.inst.pc) && row.filter.is_nonzero())
        .copied()
        .collect();

    cpu_trace.extend(unused_instructions);
    cpu_trace
}

#[cfg(test)]
mod tests {
    use plonky2::field::types::Field;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use crate::columns_view::selection;
    use crate::cpu::columns::{CpuState, Instruction};
    use crate::generation::cpu::generate_permuted_inst_trace;
    use crate::program::columns::{InstructionRow, ProgramRom};
    use crate::utils::from_u32;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_permuted_inst_trace() {
        const D: usize = 2;
        type C = Poseidon2GoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let cpu_trace: Vec<CpuState<F>> = [
            CpuState {
                inst: Instruction {
                    pc: 1,
                    ops: selection(3),
                    rs1_select: selection(2),
                    rs2_select: selection(1),
                    rd_select: selection(1),
                    imm_value: 3,
                    ..Default::default()
                },
                is_running: 1,
                ..Default::default()
            },
            CpuState {
                inst: Instruction {
                    pc: 2,
                    ops: selection(1),
                    rs1_select: selection(3),
                    rs2_select: selection(3),
                    rd_select: selection(2),
                    imm_value: 2,
                    ..Default::default()
                },
                is_running: 1,
                ..Default::default()
            },
            CpuState {
                inst: Instruction {
                    pc: 1,
                    ops: selection(3),
                    rs1_select: selection(2),
                    rs2_select: selection(1),
                    rd_select: selection(1),
                    imm_value: 3,
                    ..Default::default()
                },
                is_running: 1,
                ..Default::default()
            },
            CpuState {
                inst: Instruction {
                    pc: 1,
                    ops: selection(3),
                    rs1_select: selection(2),
                    rs2_select: selection(1),
                    rd_select: selection(1),
                    imm_value: 4,
                    ..Default::default()
                },
                is_running: 0,
                ..Default::default()
            },
        ]
        .into_iter()
        .map(|row| CpuState {
            inst: row.inst.map(from_u32),
            is_running: from_u32(row.is_running),
            ..Default::default()
        })
        .collect();

        let reduce_with_powers = |values: Vec<u64>| {
            values
                .into_iter()
                .enumerate()
                .map(|(i, x)| (1 << (i * 5)) * x)
                .sum::<u64>()
        };

        let program_trace: Vec<ProgramRom<F>> = [
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    // opcode: 3,
                    // is_op1_signed: 0,
                    // is_op2_signed: 0,
                    // rs1_select: 2,
                    // rs2_select: 1,
                    // rd_select: 1,
                    // imm_value: 3,
                    inst_data: reduce_with_powers(vec![3, 0, 0, 2, 1, 1, 3]),
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 2,
                    inst_data: reduce_with_powers(vec![1, 0, 0, 3, 3, 2, 2]),
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 3,
                    inst_data: reduce_with_powers(vec![2, 0, 0, 1, 2, 3, 1]),
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 3,
                    inst_data: reduce_with_powers(vec![2, 0, 0, 1, 2, 3, 1]),
                },
                filter: 0,
            },
        ]
        .into_iter()
        .map(|row| row.map(F::from_canonical_u64))
        .collect();

        let permuted = generate_permuted_inst_trace(&cpu_trace, &program_trace);
        let expected_permuted: Vec<ProgramRom<F>> = [
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    inst_data: reduce_with_powers(vec![3, 0, 0, 2, 1, 1, 3]),
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    inst_data: reduce_with_powers(vec![3, 0, 0, 2, 1, 1, 3]),
                },
                filter: 0,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 2,
                    inst_data: reduce_with_powers(vec![1, 0, 0, 3, 3, 2, 2]),
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 3,
                    inst_data: reduce_with_powers(vec![2, 0, 0, 1, 2, 3, 1]),
                },
                filter: 1,
            },
        ]
        .into_iter()
        .map(|row| row.map(F::from_canonical_u64))
        .collect();
        assert_eq!(permuted, expected_permuted);
    }
}
