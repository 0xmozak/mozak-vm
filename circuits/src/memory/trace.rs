use plonky2::hash::hash_types::RichField;
use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::vm::Row;

pub(crate) const OPCODE_LB: usize = 0;
pub(crate) const OPCODE_SB: usize = 1;

pub fn get_memory_inst_op<F: RichField>(inst: &Instruction) -> F {
    match inst.op {
        Op::LB => F::from_canonical_usize(OPCODE_LB),
        Op::SB => F::from_canonical_usize(OPCODE_SB),
        _ => F::ZERO,
    }
}

pub fn get_memory_inst_addr<F: RichField>(row: &Row) -> F {
    let addr = row.state
        .get_register_value(row.inst.data.rs1.into())
        .wrapping_add(row.inst.data.imm);
    F::from_canonical_u32(addr)
}

pub fn get_memory_inst_clk<F: RichField>(row: &Row) -> F {
    F::from_canonical_usize(row.state.clk)
}

pub fn get_memory_load_inst_value<F: RichField>(row: &Row) -> F {
    let state = &row.state;
    let inst = &row.inst;
    let addr = state
        .get_register_value(inst.data.rs1.into())
        .wrapping_add(inst.data.imm);
    F::from_canonical_u32(state.load_u8(addr))
}

pub fn get_memory_store_inst_value<F: RichField>(row: &Row) -> F {
    let state = &row.state;
    let inst = &row.inst;
    F::from_canonical_u32(state.get_register_value(inst.rs2.into()))
}
