use std::collections::HashSet;

use itertools::{chain, Itertools};
use mozak_vm::elf::Program;
use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::state::{Aux, State};
use mozak_vm::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::Bitshift;
use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::{CpuColumnsExtended, CpuState};
use crate::program::columns::{InstructionRow, ProgramRom};
use crate::stark::utils::transpose_trace;
use crate::utils::{from_u32, pad_trace_with_last_to_len, sign_extend};
use crate::xor::columns::XorView;

#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn generate_cpu_trace_extended<F: RichField>(
    mut cpu_trace: Vec<CpuState<F>>,
    program_rom: &[ProgramRom<F>],
) -> CpuColumnsExtended<Vec<F>> {
    let mut permuted = generate_permuted_inst_trace(&cpu_trace, program_rom);
    let len = cpu_trace.len().max(permuted.len()).next_power_of_two();
    let ori_len = permuted.len();
    permuted = pad_trace_with_last_to_len(permuted, len);
    for entry in permuted.iter_mut().skip(ori_len) {
        entry.filter = F::ZERO;
    }
    cpu_trace = pad_trace_with_last_to_len(cpu_trace, len);

    (chain!(transpose_trace(cpu_trace), transpose_trace(permuted))).collect()
}

#[allow(clippy::missing_panics_doc)]
pub fn generate_cpu_trace<F: RichField>(
    program: &Program,
    record: &ExecutionRecord,
) -> Vec<CpuState<F>> {
    let mut trace: Vec<CpuState<F>> = vec![];
    let ExecutionRecord {
        executed,
        last_state,
    } = record;
    let last_row = &[Row {
        state: last_state.clone(),
        // `Aux` has auxiliary information about an executed CPU cycle.
        // The last state is the final state after the last execution.  Thus naturally it has no
        // associated auxiliarye execution information. We use a dummy aux to make the row
        // generation work, but we could refactor to make this unnecessary.
        aux: executed.last().unwrap().aux.clone(),
    }];

    for Row { state, aux } in chain![executed, last_row] {
        let inst = state.current_instruction(program);
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
            bitshift: Bitshift::from(0).map(F::from_canonical_u64),
            xor: generate_xor_row(&inst, state),

            ..CpuState::default()
        };

        for j in 0..32 {
            row.regs[j as usize] = from_u32(state.get_register_value(j));
        }

        generate_mul_row(&mut row, &inst, aux);
        generate_divu_row(&mut row, &inst, aux);
        generate_sign_handling(&mut row, aux);
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

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::similar_names)]
fn generate_mul_row<F: RichField>(row: &mut CpuState<F>, inst: &Instruction, aux: &Aux) {
    // let op1_full_range = sign_extend(row.is_op1_signed().is_nonzero(), aux.op1);
    // let op2_full_range = sign_extend(row.is_op2_signed().is_nonzero(), aux.op2);
    let multiplier = if let Op::SLL = inst.op {
        let shift_amount = aux.op2 & 0b1_1111;
        let shift_power = 1_u32 << shift_amount;
        row.bitshift = Bitshift {
            amount: shift_amount,
            multiplier: shift_power,
        }
        .map(from_u32);
        shift_power
    } else {
        aux.op2
    };

    row.multiplier = from_u32(multiplier);
    let (low, high) = aux.op1.widening_mul(multiplier);
    row.product_low_bits = from_u32(low);
    row.product_high_bits = from_u32(high);

    // row.multiplier = F::from_noncanonical_i64(multiplier);
    // let product = i128::from(op1_full_range).wrapping_mul(multiplier.into());
    // row.product_low_bits = F::from_canonical_u32(product as u32);
    // row.product_high_bits = F::from_canonical_u32((product >> 32) as u32);

    // Prove that the high limb is different from `u32::MAX`:
    let high_diff: F = from_u32(u32::MAX - high);
    // let high_diff: F = F::from_canonical_u32(u32::MAX) - row.product_high_bits;
    row.product_high_diff_inv = high_diff.try_inverse().unwrap_or_default();
}

#[allow(clippy::cast_possible_wrap)]
fn generate_divu_row<F: RichField>(row: &mut CpuState<F>, inst: &Instruction, aux: &Aux) {
    let dividend = aux.op1;

    let divisor = if let Op::SRL = inst.op {
        let shift_amount = aux.op2 & 0x1F;
        let shift_power = 1_u32 << shift_amount;
        row.bitshift = Bitshift {
            amount: shift_amount,
            multiplier: shift_power,
        }
        .map(from_u32);
        shift_power
    } else {
        aux.op2
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
#[allow(clippy::cast_lossless)]
fn generate_sign_handling<F: RichField>(row: &mut CpuState<F>, aux: &Aux) {
    let op1_full_range = sign_extend(row.is_op1_signed().is_nonzero(), aux.op1);
    let op2_full_range = sign_extend(row.is_op2_signed().is_nonzero(), aux.op2);

    row.op1_sign_bit = F::from_bool(op1_full_range < 0);
    row.op2_sign_bit = F::from_bool(op2_full_range < 0);

    row.less_than = F::from_bool(op1_full_range < op2_full_range);
    let abs_diff = op1_full_range.abs_diff(op2_full_range);
    row.abs_diff = F::from_noncanonical_u64(abs_diff);
}

fn generate_xor_row<F: RichField>(inst: &Instruction, state: &State) -> XorView<F> {
    let a = match inst.op {
        Op::AND | Op::OR | Op::XOR => state.get_register_value(inst.args.rs1),
        Op::SRL | Op::SLL => 0b1_1111,
        _ => 0,
    };
    let b = state
        .get_register_value(inst.args.rs2)
        .wrapping_add(inst.args.imm);
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
        .filter(|row| !used_pcs.contains(&row.inst.pc))
        .copied()
        .collect();

    cpu_trace.extend(unused_instructions);
    cpu_trace
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::columns_view::selection;
    use crate::cpu::columns::{CpuState, Instruction};
    use crate::generation::cpu::generate_permuted_inst_trace;
    use crate::program::columns::{InstructionRow, ProgramRom};
    use crate::utils::from_u32;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_permuted_inst_trace() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
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
                    ops: selection(4),
                    rs1_select: selection(4),
                    rs2_select: selection(4),
                    rd_select: selection(4),
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

        let program_trace: Vec<ProgramRom<F>> = [
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    opcode: 3,
                    rs1: 2,
                    rs2: 1,
                    rd: 1,
                    imm: 3,
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 2,
                    opcode: 1,
                    rs1: 3,
                    rs2: 3,
                    rd: 2,
                    imm: 2,
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 3,
                    opcode: 2,
                    rs1: 1,
                    rs2: 2,
                    rd: 3,
                    imm: 1,
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    opcode: 3,
                    rs1: 3,
                    rs2: 3,
                    rd: 3,
                    imm: 3,
                },
                filter: 0,
            },
        ]
        .into_iter()
        .map(|row| row.map(from_u32))
        .collect();

        let permuted = generate_permuted_inst_trace(&cpu_trace, &program_trace);
        let expected_permuted: Vec<ProgramRom<F>> = [
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    opcode: 3,
                    rs1: 2,
                    rs2: 1,
                    rd: 1,
                    imm: 3,
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 1,
                    opcode: 3,
                    rs1: 2,
                    rs2: 1,
                    rd: 1,
                    imm: 3,
                },
                filter: 0,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 2,
                    opcode: 1,
                    rs1: 3,
                    rs2: 3,
                    rd: 2,
                    imm: 2,
                },
                filter: 1,
            },
            ProgramRom {
                inst: InstructionRow {
                    pc: 3,
                    opcode: 2,
                    rs1: 1,
                    rs2: 2,
                    rd: 3,
                    imm: 1,
                },
                filter: 1,
            },
        ]
        .into_iter()
        .map(|row| row.map(from_u32))
        .collect();
        assert_eq!(permuted, expected_permuted);
    }
}
