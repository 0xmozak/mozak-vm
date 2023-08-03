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
        #[tarpaulin::skip]
        _ => F::ZERO,
    }
}

#[must_use]
pub fn get_memory_inst_addr<F: Field>(row: &Row) -> F {
    F::from_canonical_u32(row.aux.mem_addr.unwrap_or_default())
}

#[must_use]
pub fn get_memory_inst_clk<F: Field>(row: &Row) -> F { F::from_canonical_u64(row.state.clk) }

#[must_use]
pub fn get_memory_load_inst_value<F: Field>(row: &Row) -> F {
    F::from_canonical_u32(row.aux.dst_val)
}

#[must_use]
pub fn get_memory_store_inst_value<F: Field>(row: &Row) -> F {
    F::from_canonical_u32(row.aux.dst_val)
}
