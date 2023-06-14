use mozak_vm::instruction::{Instruction, Op};
use mozak_vm::vm::Row;
use plonky2::field::types::Field;

pub(crate) const OPCODE_LB: usize = 0;
pub(crate) const OPCODE_SB: usize = 1;

#[must_use]
pub fn get_memory_inst_op<F: Field>(inst: &Instruction) -> F {
    match inst.op {
        Op::LB => F::from_canonical_usize(OPCODE_LB),
        Op::SB => F::from_canonical_usize(OPCODE_SB),
        _ => F::ZERO,
    }
}

#[must_use]
pub fn get_memory_inst_addr<F: Field>(row: &Row) -> F {
    let addr = row
        .state
        .get_register_value(row.inst.data.rs1.into())
        .wrapping_add(row.inst.data.imm);
    F::from_canonical_u32(addr)
}

#[must_use]
pub fn get_memory_inst_clk<F: Field>(row: &Row) -> F {
    F::from_canonical_u64(row.state.clk)
}

#[must_use]
pub fn get_memory_load_inst_value<F: Field>(row: &Row) -> F {
    let state = &row.state;
    let inst = &row.inst;
    let addr = state
        .get_register_value(inst.data.rs1.into())
        .wrapping_add(inst.data.imm);
    F::from_canonical_u8(state.load_u8(addr))
}

#[must_use]
pub fn get_memory_store_inst_value<F: Field>(row: &Row) -> F {
    let state = &row.state;
    let inst = &row.inst;
    F::from_canonical_u32(state.get_register_value(inst.data.rs2.into()))
}
