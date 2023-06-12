pub mod memory;

use mozak_vm::vm::Row;
use plonky2::{
    field::{extension::Extendable, polynomial::PolynomialValues},
    hash::hash_types::RichField,
};

use crate::generation::memory::generate_memory_trace;
use crate::stark::utils::trace_to_poly_values;

pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    step_rows: Vec<Row>,
) -> [Vec<PolynomialValues<F>>; 1] {
    let memory_rows = generate_memory_trace::<F>(step_rows);
    let memory_trace = trace_to_poly_values(memory_rows);
    [memory_trace]
}
