use itertools::EitherOrBoth::{self, Both, Left, Right};
use itertools::{chain, merge_join_by};
use mozak_vm::elf::Program;
use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::state::{Aux, State};
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::bitshift::columns::Bitshift;
use crate::bitwise::columns::XorView;
use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::{CpuColumnsExtended, CpuState};
use crate::program::columns::{InstColumnsView, ProgramColumnsView};
use crate::stark::utils::transpose_trace;
use crate::utils::{from_u32, pad_trace_with_default_to_len};

#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn generate_cpu_trace_extended<F: RichField>(
    cpu_trace: Vec<CpuState<F>>,
    program_trace: &[ProgramColumnsView<F>],
) -> CpuColumnsExtended<Vec<F>> {
    let extended = generate_permuted_inst_trace(&cpu_trace, program_trace);
    let len = cpu_trace.len().max(extended.len()).next_power_of_two();
    let extended = pad_trace_with_default_to_len(extended, len);
    let cpu_trace = pad_trace_with_default_to_len(cpu_trace, len);

    (chain!(transpose_trace(cpu_trace), transpose_trace(extended))).collect()
}

pub fn generate_cpu_trace<F: RichField>(program: &Program, step_rows: &[Row]) -> Vec<CpuState<F>> {
    let mut trace: Vec<CpuState<F>> = vec![];

    for Row { state, aux } in step_rows {
        let inst = state.current_instruction(program);
        let mut row = CpuState {
            clk: F::from_noncanonical_u64(state.clk),
            inst: cpu_cols::Instruction::from((state.get_pc(), inst)).map(from_u32),
            op1_value: from_u32(aux.op1),
            // OP2_VALUE is the sum of the value of the second operand register and the
            // immediate value.
            op2_value: from_u32(aux.op2),
            // NOTE: Updated value of DST register is next step.
            dst_value: from_u32(aux.dst_val),
            halt: from_u32(u32::from(aux.will_halt)),
            // Valid defaults for the powers-of-two gadget.
            // To be overridden by users of the gadget.
            // TODO(Matthias): find a way to make either compiler or runtime complain
            // if we have two (conflicting) users in the same row.
            bitshift: Bitshift::from(0).map(F::from_canonical_u64),
            xor: generate_bitwise_row(&inst, state),

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
    let diff = row.op1_value - row.op2_value;
    let diff_inv = diff.try_inverse().unwrap_or_default();

    row.cmp_diff_inv = diff_inv;
    row.not_diff = F::ONE - diff * diff_inv;
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::similar_names)]
fn generate_mul_row<F: RichField>(row: &mut CpuState<F>, inst: &Instruction, aux: &Aux) {
    if !matches!(inst.op, Op::MUL | Op::MULHU | Op::SLL) {
        return;
    }
    let multiplier = if let Op::SLL = inst.op {
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

    row.multiplier = from_u32(multiplier);
    let (low, high) = aux.op1.widening_mul(multiplier);
    row.product_low_bits = from_u32(low);
    row.product_high_bits = from_u32(high);

    // Prove that the high limb is different from `u32::MAX`:
    let high_diff: F = from_u32(u32::MAX - high);
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
    let is_signed: bool = row.is_signed().is_nonzero();
    let embed = if is_signed {
        |x: u32| x as i32 as i64
    } else {
        |x: u32| x as i64
    };

    let op1_full_range = embed(aux.op1);
    let op2_full_range = embed(aux.op2);

    row.op1_sign_bit = F::from_bool(op1_full_range < 0);
    row.op2_sign_bit = F::from_bool(op2_full_range < 0);

    row.less_than = F::from_bool(op1_full_range < op2_full_range);
    let abs_diff = op1_full_range.abs_diff(op2_full_range);
    row.abs_diff = F::from_noncanonical_u64(abs_diff);
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
    trace: &[CpuState<F>],
    program_rom: &[ProgramColumnsView<F>],
) -> Vec<ProgramColumnsView<F>> {
    let mut trace: Vec<_> = trace
        .iter()
        .filter(|row| row.halt == F::ZERO)
        .map(|row| row.inst)
        .collect();
    trace.sort_by_key(|inst| inst.pc.to_noncanonical_u64());
    let mut trace = merge_join_by(trace, program_rom, |exec, rom| {
        exec.pc
            .to_noncanonical_u64()
            .cmp(&rom.inst.pc.to_noncanonical_u64())
    })
    .collect::<Vec<_>>();
    trace.sort_by_key(EitherOrBoth::has_left);
    trace
        .into_iter()
        .map(|row| match row {
            Left(inst) | Both(inst, _) => ProgramColumnsView {
                filter: F::from_bool(row.has_right()),
                inst: InstColumnsView::from(inst),
            },
            Right(&inst_view) => inst_view,
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::columns_view::selection;
    use crate::cpu::columns::{CpuState, Instruction};
    use crate::generation::cpu::generate_permuted_inst_trace;
    use crate::program::columns::{InstColumnsView, ProgramColumnsView};
    use crate::utils::from_u32;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_pad_permuted_inst_trace() {
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
                halt: 0,
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
                halt: 0,
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
                halt: 0,
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
                halt: 1,
                ..Default::default()
            },
        ]
        .into_iter()
        .map(|row| CpuState {
            inst: row.inst.map(from_u32),
            halt: from_u32(row.halt),
            ..Default::default()
        })
        .collect();

        let program_trace: Vec<ProgramColumnsView<F>> = [
            ProgramColumnsView {
                inst: InstColumnsView {
                    pc: 1,
                    opcode: 3,
                    rs1: 2,
                    rs2: 1,
                    rd: 1,
                    imm: 3,
                },
                filter: 1,
            },
            ProgramColumnsView {
                inst: InstColumnsView {
                    pc: 2,
                    opcode: 1,
                    rs1: 3,
                    rs2: 3,
                    rd: 2,
                    imm: 2,
                },
                filter: 1,
            },
            ProgramColumnsView {
                inst: InstColumnsView {
                    pc: 3,
                    opcode: 2,
                    rs1: 1,
                    rs2: 2,
                    rd: 3,
                    imm: 1,
                },
                filter: 1,
            },
            ProgramColumnsView {
                inst: InstColumnsView {
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

        let extended = generate_permuted_inst_trace(&cpu_trace, &program_trace);
        let expected_extened: Vec<ProgramColumnsView<F>> = [
            ProgramColumnsView {
                inst: InstColumnsView {
                    pc: 1,
                    opcode: 3,
                    rs1: 2,
                    rs2: 1,
                    rd: 1,
                    imm: 3,
                },
                filter: 1,
            },
            ProgramColumnsView {
                inst: InstColumnsView {
                    pc: 1,
                    opcode: 3,
                    rs1: 2,
                    rs2: 1,
                    rd: 1,
                    imm: 3,
                },
                filter: 0,
            },
            ProgramColumnsView {
                inst: InstColumnsView {
                    pc: 2,
                    opcode: 1,
                    rs1: 3,
                    rs2: 3,
                    rd: 2,
                    imm: 2,
                },
                filter: 1,
            },
            ProgramColumnsView {
                inst: InstColumnsView {
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
        assert_eq!(extended, expected_extened);
    }
}
