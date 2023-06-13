use mozak_vm::instruction::Instruction;
use mozak_vm::vm::Row;
use plonky2::field::types::Field;

use crate::utils::from_;

pub fn get_memory_inst_op<F: Field>(inst: &Instruction) -> F {
    from_(inst.op as u32)
}

pub fn get_memory_inst_addr<F: Field>(row: &Row) -> F {
    from_(
        row.state
            .get_register_value(row.inst.data.rs1.into())
            .wrapping_add(row.inst.data.imm),
    )
}

pub fn get_memory_inst_clk<F: Field>(row: &Row) -> F {
    from_(row.state.clk)
}

pub fn get_memory_load_inst_value<F: Field>(row: &Row) -> F {
    let state = &row.state;
    let inst = &row.inst;
    let addr = state
        .get_register_value(inst.data.rs1.into())
        .wrapping_add(inst.data.imm);
    F::from_canonical_u8(state.load_u8(addr))
}

pub fn get_memory_store_inst_value<F: Field>(row: &Row) -> F {
    let state = &row.state;
    let inst = &row.inst;
    F::from_canonical_u32(state.get_register_value(inst.data.rs2.into()))
}
