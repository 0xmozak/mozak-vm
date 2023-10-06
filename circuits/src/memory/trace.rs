use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

#[must_use]
pub fn get_memory_inst_addr<F: RichField>(row: &Row<F>) -> F {
    F::from_canonical_u32(row.aux.mem_addr.unwrap_or_default())
}

#[must_use]
pub fn get_memory_inst_clk<F: RichField>(row: &Row<F>) -> F { F::from_canonical_u64(row.state.clk) }
