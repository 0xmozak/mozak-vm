//! This module is responsible for populating the the Stark Tables with the
//! appropriate values based on the [`Program`] and [`ExecutionRecord`].

pub mod bitshift;
pub mod cpu;
pub mod instruction;
pub mod memory;
pub mod memoryinit;
pub mod program;
pub mod rangecheck;
pub mod rangecheck_limb;
pub mod register;
pub mod registerinit;
pub mod xor;

use std::borrow::Borrow;
use std::fmt::Display;

use itertools::Itertools;
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;
use plonky2::util::transpose;
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

use self::bitshift::generate_shift_amount_trace;
use self::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
use self::memory::generate_memory_trace;
use self::memoryinit::generate_memory_init_trace;
use self::rangecheck::generate_rangecheck_trace;
use self::rangecheck_limb::generate_rangecheck_limb_trace;
use self::xor::generate_xor_trace;
use crate::bitshift::stark::BitshiftStark;
use crate::cpu::stark::CpuStark;
use crate::generation::program::generate_program_rom_trace;
use crate::memory::stark::MemoryStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_limb::stark::RangeCheckLimbStark;
use crate::stark::mozak_stark::{MozakStark, PublicInputs, NUM_TABLES};
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};
use crate::xor::stark::XorStark;

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    record: &ExecutionRecord,
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(program, record);
    let xor_rows = generate_xor_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    let program_rows = generate_program_rom_trace(program);
    let memory_init_rows = generate_memory_init_trace(program);
    let memory_rows = generate_memory_trace(program, &record.executed);
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);
    let rangecheck_limb_rows = generate_rangecheck_limb_trace(&cpu_rows, &rangecheck_rows);

    let cpu_trace = trace_to_poly_values(generate_cpu_trace_extended(cpu_rows, &program_rows));
    let rangecheck_trace = trace_rows_to_poly_values(rangecheck_rows);
    let xor_trace = trace_rows_to_poly_values(xor_rows);
    let shift_amount_trace = trace_rows_to_poly_values(shift_amount_rows);
    let program_trace = trace_rows_to_poly_values(program_rows);
    let memory_trace = trace_rows_to_poly_values(memory_rows);
    let memory_init_trace = trace_rows_to_poly_values(memory_init_rows);
    let rangecheck_limb_trace = trace_rows_to_poly_values(rangecheck_limb_rows);
    [
        cpu_trace,
        rangecheck_trace,
        xor_trace,
        shift_amount_trace,
        program_trace,
        memory_trace,
        memory_init_trace,
        rangecheck_limb_trace,
    ]
}

#[must_use]
pub fn transpose_polys<
    F: RichField + Extendable<D> + PackedField,
    const D: usize,
    S: Stark<F, D>,
>(
    cols: Vec<PolynomialValues<F>>,
) -> Vec<[F; S::COLUMNS]> {
    transpose(
        &cols
            .into_iter()
            .map(|PolynomialValues { values }| values)
            .collect_vec(),
    )
    .into_iter()
    .map(|row| row.try_into().unwrap())
    .collect_vec()
}

pub fn debug_traces<F: RichField + Extendable<D>, const D: usize>(
    traces_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    mozak_stark: &MozakStark<F, D>,
    public_inputs: &PublicInputs<F>,
) where
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); RangeCheckStark::<F, D>::PUBLIC_INPUTS]:,
    [(); XorStark::<F, D>::COLUMNS]:,
    [(); BitshiftStark::<F, D>::COLUMNS]:,
    // [(); ProgramStark::<F, D>::COLUMNS]:,
    [(); MemoryStark::<F, D>::COLUMNS]:,
    [(); MemoryInitStark::<F, D>::COLUMNS]:,
    [(); RangeCheckLimbStark::<F, D>::COLUMNS]:, {
    let [cpu, rangecheck, xor, shift_amount, program, memory, memory_init, rangecheck_limb]: &[Vec<
        PolynomialValues<F>,
    >;
        NUM_TABLES] = traces_poly_values;

    assert!([
        // Program ROM
        debug_single_trace::<F, D, ProgramStark<F, D>>(&mozak_stark.program_stark, program, &[],),
        // CPU
        debug_single_trace::<F, D, CpuStark<F, D>>(
            &mozak_stark.cpu_stark,
            cpu,
            public_inputs.borrow(),
        ),
        // Range check
        debug_single_trace::<F, D, RangeCheckStark<F, D>>(
            &mozak_stark.rangecheck_stark,
            rangecheck,
            &[],
        ),
        // Xor
        debug_single_trace::<F, D, XorStark<F, D>>(&mozak_stark.xor_stark, xor, &[]),
        // Bitshift
        debug_single_trace::<F, D, BitshiftStark<F, D>>(
            &mozak_stark.shift_amount_stark,
            shift_amount,
            &[],
        ),
        // Memory
        debug_single_trace::<F, D, MemoryStark<F, D>>(&mozak_stark.memory_stark, memory, &[],),
        // MemoryInit
        debug_single_trace::<F, D, MemoryInitStark<F, D>>(
            &mozak_stark.memory_init_stark,
            memory_init,
            &[],
        ),
        debug_single_trace::<F, D, RangeCheckLimbStark<F, D>>(
            &mozak_stark.rangecheck_limb_stark,
            rangecheck_limb,
            &[],
        ),
    ]
    .into_iter()
    .all(|x| x));
}

pub fn debug_single_trace<
    F: RichField + Extendable<D>,
    const D: usize,
    S: Stark<F, D> + Display,
>(
    stark: &S,
    trace_rows: &[PolynomialValues<F>],
    public_inputs: &[F; S::PUBLIC_INPUTS],
) -> bool
where
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:, {
    transpose_polys::<F, D, S>(trace_rows.to_vec())
        .iter()
        .enumerate()
        .circular_tuple_windows()
        .map(|((lv_row, lv), (nv_row, nv))| {
            let mut consumer = ConstraintConsumer::new_debug_api(lv_row == 0, nv_row == 0);
            stark.eval_packed_generic(
                StarkEvaluationVars {
                    local_values: lv,
                    next_values: nv,
                    public_inputs,
                },
                &mut consumer,
            );
            if consumer.debug_api_has_constraint_failed() {
                println!("Debug constraints for {stark}");
                println!("lv-row[{lv_row}] - values: {lv:?}");
                println!("nv-row[{nv_row}] - values: {nv:?}");
                false
            } else {
                true
            }
        })
        .all(|x| x)
}
