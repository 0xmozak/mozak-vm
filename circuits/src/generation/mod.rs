pub mod cpu;
pub mod memory;

use mozak_vm::vm::Row;
use plonky2::{
    field::{extension::Extendable, polynomial::PolynomialValues},
    hash::hash_types::RichField,
};

use self::cpu::generate_cpu_trace;
use crate::stark::utils::trace_to_poly_values;

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    step_rows: &[Row],
) -> [Vec<PolynomialValues<F>>; 1] {
    let cpu_rows = generate_cpu_trace::<F>(step_rows);
    let cpu_trace = trace_to_poly_values(cpu_rows);
    [cpu_trace]
}
