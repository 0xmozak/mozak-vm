use itertools::izip;
use mozak_runner::instruction::{Instruction, Op};
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns;
use crate::program::columns::ProgramRom;

impl From<(u32, Instruction)> for columns::Instruction<u32> {
    fn from((pc, inst): (u32, Instruction)) -> Self {
        let mut cols: columns::Instruction<u32> = Self {
            pc,
            imm_value: inst.args.imm,
            is_op1_signed: matches!(
                inst.op,
                Op::SLT | Op::DIV | Op::REM | Op::MULH | Op::MULHSU | Op::BLT | Op::BGE | Op::SRA
            )
            .into(),
            is_op2_signed: matches!(
                inst.op,
                Op::SLT | Op::DIV | Op::REM | Op::MULH | Op::BLT | Op::BGE
            )
            .into(),
            is_dst_signed: matches!(inst.op, Op::LB | Op::LH).into(),
            ..Self::default()
        };
        *match inst.op {
            Op::ADD => &mut cols.ops.add,
            Op::LBU | Op::LB => &mut cols.ops.lb,
            Op::LH | Op::LHU => &mut cols.ops.lh,
            Op::LW => &mut cols.ops.lw,
            Op::SLL => &mut cols.ops.sll,
            Op::SLT | Op::SLTU => &mut cols.ops.slt,
            Op::SB => &mut cols.ops.sb,
            Op::SH => &mut cols.ops.sh,
            Op::SW => &mut cols.ops.sw,
            Op::SRL => &mut cols.ops.srl,
            Op::SRA => &mut cols.ops.sra,
            Op::SUB => &mut cols.ops.sub,
            Op::DIV | Op::DIVU => &mut cols.ops.div,
            Op::REM | Op::REMU => &mut cols.ops.rem,
            Op::MUL => &mut cols.ops.mul,
            Op::MULH | Op::MULHU | Op::MULHSU => &mut cols.ops.mulh,
            Op::JALR => &mut cols.ops.jalr,
            Op::BEQ => &mut cols.ops.beq,
            Op::BNE => &mut cols.ops.bne,
            Op::BLT | Op::BLTU => &mut cols.ops.blt,
            Op::BGE | Op::BGEU => &mut cols.ops.bge,
            Op::ECALL => &mut cols.ops.ecall,
            Op::XOR => &mut cols.ops.xor,
            Op::OR => &mut cols.ops.or,
            Op::AND => &mut cols.ops.and,
        } = 1;
        cols.rs1_selected = u32::from(inst.args.rs1);
        cols.rs2_selected = u32::from(inst.args.rs2);
        cols.rd_selected = u32::from(inst.args.rd);
        cols
    }
}

pub fn ascending_sum<F: RichField, I: IntoIterator<Item = F>>(cs: I) -> F {
    izip![(0..).map(F::from_canonical_u64), cs]
        .map(|(i, x)| i * x)
        .sum()
}

pub fn reduce_with_powers<F: RichField, I: IntoIterator<Item = F>>(terms: I, alpha: u64) -> F {
    izip!((0..).map(|i| F::from_canonical_u64(alpha.pow(i))), terms)
        .map(|(base, val)| base * val)
        .sum()
}

impl<F: RichField> From<columns::Instruction<F>> for ProgramRom<F> {
    fn from(inst: columns::Instruction<F>) -> Self {
        Self {
            pc: inst.pc,
            inst_data: reduce_with_powers(
                [
                    ascending_sum(inst.ops),
                    inst.is_op1_signed,
                    inst.is_op2_signed,
                    inst.rs1_selected,
                    inst.rs2_selected,
                    inst.rd_selected,
                    inst.imm_value,
                ],
                1 << 5,
            ),
        }
    }
}
