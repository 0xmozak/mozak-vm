use itertools::izip;
use mozak_vm::instruction::{Instruction, Op};
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns;
use crate::program::columns::InstructionRow;

impl From<(u32, Instruction)> for columns::Instruction<u32> {
    fn from((pc, inst): (u32, Instruction)) -> Self {
        let mut cols: columns::Instruction<u32> = Self {
            pc,
            imm_value: inst.args.imm,
            branch_target: inst.args.branch_target,
            ..Self::default()
        };
        *(match inst.op {
            Op::ADD => &mut cols.ops.add,
            Op::LBU => &mut cols.ops.lbu,
            Op::SLL => &mut cols.ops.sll,
            Op::SLT => &mut cols.ops.slt,
            Op::SLTU => &mut cols.ops.sltu,
            Op::SB => &mut cols.ops.sb,
            Op::SRL => &mut cols.ops.srl,
            Op::SUB => &mut cols.ops.sub,
            Op::DIVU => &mut cols.ops.divu,
            Op::REMU => &mut cols.ops.remu,
            Op::MUL => &mut cols.ops.mul,
            Op::MULHU => &mut cols.ops.mulhu,
            Op::JALR => &mut cols.ops.jalr,
            Op::BEQ => &mut cols.ops.beq,
            Op::BNE => &mut cols.ops.bne,
            Op::BLT => &mut cols.ops.blt,
            Op::BLTU => &mut cols.ops.bltu,
            Op::BGE => &mut cols.ops.bge,
            Op::BGEU => &mut cols.ops.bgeu,
            Op::ECALL => &mut cols.ops.ecall,
            Op::XOR => &mut cols.ops.xor,
            Op::OR => &mut cols.ops.or,
            Op::AND => &mut cols.ops.and,
            #[tarpaulin::skip]
            _ => unreachable!(),
        }) = 1;
        cols.rs1_select[inst.args.rs1 as usize] = 1;
        cols.rs2_select[inst.args.rs2 as usize] = 1;
        cols.rd_select[inst.args.rd as usize] = 1;
        cols
    }
}

pub fn ascending_sum<F: RichField, I: IntoIterator<Item = F>>(cs: I) -> F {
    izip![(0..).map(F::from_canonical_u64), cs]
        .map(|(i, x)| i * x)
        .sum()
}

impl<F: RichField> From<columns::Instruction<F>> for InstructionRow<F> {
    fn from(inst: columns::Instruction<F>) -> Self {
        Self {
            pc: inst.pc,
            opcode: ascending_sum(inst.ops),
            rs1: ascending_sum(inst.rs1_select),
            rs2: ascending_sum(inst.rs2_select),
            rd: ascending_sum(inst.rd_select),
            imm: inst.imm_value,
        }
    }
}
