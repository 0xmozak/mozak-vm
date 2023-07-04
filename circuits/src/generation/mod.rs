pub mod cpu;
pub mod memory;
pub mod rangecheck;

use mozak_vm::vm::Row;
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;

use self::{cpu::generate_cpu_trace, rangecheck::generate_rangecheck_trace};
use crate::stark::utils::trace_to_poly_values;

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    step_rows: &[Row],
) -> [Vec<PolynomialValues<F>>; 2] {
    let cpu_rows = generate_cpu_trace::<F>(step_rows);
    let cpu_trace = trace_to_poly_values(cpu_rows);

    let rangecheck_rows = generate_rangecheck_trace::<F>(step_rows);
    let rangecheck_trace = trace_to_poly_values(rangecheck_rows);
    [cpu_trace, rangecheck_trace]
}
