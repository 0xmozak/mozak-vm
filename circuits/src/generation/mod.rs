pub mod bitwise;
pub mod cpu;
pub mod memory;
pub mod rangecheck;

use mozak_vm::vm::Row;
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;

use self::bitwise::generate_bitwise_trace;
use self::cpu::generate_cpu_trace;
use self::rangecheck::generate_rangecheck_trace;
use crate::cpu::columns::{self as cpu_cols};
use crate::program::{MAP, NUM_PROGRAM_COLS};
use crate::stark::mozak_stark::NUM_TABLES;
use crate::stark::utils::trace_to_poly_values;

/// Generates a program trace from CPU traces.
///
/// Note: The ideal source for generating the program trace should be ELF file
/// instructions instead of CPU traces. This approach would require a
/// substantial refactoring, including the separation of local opcode decoding
/// from CPU trace generation.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_program_trace<F: RichField>(
    cpu_trace: &[Vec<F>; cpu_cols::NUM_CPU_COLS],
) -> [Vec<F>; NUM_PROGRAM_COLS] {
    let mut trace: [Vec<F>; NUM_PROGRAM_COLS] = Default::default();
    let trace_len = cpu_trace[cpu_cols::MAP.pc].len();
    for vec in &mut trace {
        vec.resize(trace_len, F::ZERO);
    }
    for i in 0..trace_len {
        trace[MAP.program_is_inst][i] = F::ONE;
        trace[MAP.program_pc][i] = cpu_trace[cpu_cols::MAP.pc][i];
    }
    // trace[MAP.program_is_inst][0] = F::ZERO;
    dbg!(trace[MAP.program_is_inst].clone());
    trace
}

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    step_rows: &[Row],
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(step_rows);
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows);
    let bitwise_rows = generate_bitwise_trace(step_rows, &cpu_rows);
    let program_rows = generate_program_trace(&cpu_rows);

    let cpu_trace = trace_to_poly_values(cpu_rows);
    let rangecheck_trace = trace_to_poly_values(rangecheck_rows);
    let bitwise_trace = trace_to_poly_values(bitwise_rows);
    let program_trace = trace_to_poly_values(program_rows);
    [cpu_trace, rangecheck_trace, bitwise_trace, program_trace]
}
