use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::state::State;
use mozak_vm::vm::Row;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::cpu::columns::InstructionView;
use crate::utils::{from_u32, pad_trace};

impl From<(u32, Instruction)> for InstructionView<u32> {
    fn from((pc, inst): (u32, Instruction)) -> Self {
        let mut cols: InstructionView<u32> = Self {
            pc,
            imm_value: inst.args.imm,
            branch_target: inst.args.branch_target,
            ..Self::default()
        };
        *(match inst.op {
            Op::ADD => &mut cols.ops.add,
            Op::SLL => &mut cols.ops.sll,
            Op::SLT => &mut cols.ops.slt,
            Op::SLTU => &mut cols.ops.sltu,
            Op::SRL => &mut cols.ops.srl,
            Op::SUB => &mut cols.ops.sub,
            Op::DIVU => &mut cols.ops.divu,
            Op::REMU => &mut cols.ops.remu,
            Op::MUL => &mut cols.ops.mul,
            Op::MULHU => &mut cols.ops.mulhu,
            Op::JALR => &mut cols.ops.jalr,
            Op::BEQ => &mut cols.ops.beq,
            Op::BNE => &mut cols.ops.bne,
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
