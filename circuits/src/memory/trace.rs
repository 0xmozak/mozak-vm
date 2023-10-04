use mozak_runner::vm::Row;
use plonky2::field::types::Field;

#[must_use]
pub fn get_memory_inst_addr<F: Field>(row: &Row) -> F {
    F::from_canonical_u32(row.aux.mem.unwrap_or_default().0)
}

#[must_use]
pub fn get_memory_inst_clk<F: Field>(row: &Row) -> F { F::from_canonical_u64(row.state.clk) }
