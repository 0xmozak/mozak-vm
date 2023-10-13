//! This module is responsible for populating the the Stark Tables with the
//! appropriate values based on the [`Program`] and [`ExecutionRecord`].

pub mod bitshift;
pub mod cpu;
pub mod fullword_memory;
pub mod halfword_memory;
pub mod instruction;
pub mod memory;
pub mod memoryinit;
pub mod poseidon2;
pub mod poseidon2_sponge;
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
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::Stark;

use self::bitshift::generate_shift_amount_trace;
use self::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
use self::fullword_memory::generate_fullword_memory_trace;
use self::halfword_memory::generate_halfword_memory_trace;
use self::memory::generate_memory_trace;
use self::memoryinit::generate_memory_init_trace;
use self::poseidon2_sponge::generate_poseidon2_sponge_trace;
use self::rangecheck::generate_rangecheck_trace;
use self::rangecheck_limb::generate_rangecheck_limb_trace;
use self::register::generate_register_trace;
use self::registerinit::generate_register_init_trace;
use self::xor::generate_xor_trace;
use crate::bitshift::stark::BitshiftStark;
use crate::cpu::stark::CpuStark;
use crate::generation::poseidon2::generate_poseidon2_trace;
use crate::generation::program::generate_program_rom_trace;
use crate::memory::stark::MemoryStark;
use crate::memory_fullword::stark::FullWordMemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::poseidon2::stark::Poseidon2_12Stark;
use crate::poseidon2_sponge::stark::Poseidon2SpongeStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_limb::stark::RangeCheckLimbStark;
use crate::register::stark::RegisterStark;
use crate::registerinit::stark::RegisterInitStark;
use crate::stark::mozak_stark::{MozakStark, PublicInputs, NUM_TABLES};
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};
use crate::xor::stark::XorStark;

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    record: &ExecutionRecord<F>,
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(program, record);
    let xor_rows = generate_xor_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    let program_rows = generate_program_rom_trace(program);
    let memory_init_rows = generate_memory_init_trace(program);
    let halfword_memory_rows = generate_halfword_memory_trace(program, &record.executed);
    let fullword_memory_rows = generate_fullword_memory_trace(program, &record.executed);
    let memory_rows = generate_memory_trace(
        program,
        &record.executed,
        &memory_init_rows,
        &halfword_memory_rows,
        &fullword_memory_rows,
    );
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);
    let rangecheck_limb_rows = generate_rangecheck_limb_trace(&cpu_rows, &rangecheck_rows);
    let register_init_rows = generate_register_init_trace::<F>();
    let register_rows = generate_register_trace::<F>(program, record);
    let poseiden2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);
    let poseidon2_rows = generate_poseidon2_trace(&record.executed);

    let cpu_trace = trace_to_poly_values(generate_cpu_trace_extended(cpu_rows, &program_rows));
    let rangecheck_trace = trace_rows_to_poly_values(rangecheck_rows);
    let xor_trace = trace_rows_to_poly_values(xor_rows);
    let shift_amount_trace = trace_rows_to_poly_values(shift_amount_rows);
    let program_trace = trace_rows_to_poly_values(program_rows);
    let memory_trace = trace_rows_to_poly_values(memory_rows);
    let memory_init_trace = trace_rows_to_poly_values(memory_init_rows);
    let rangecheck_limb_trace = trace_rows_to_poly_values(rangecheck_limb_rows);
    let poseidon2_trace = trace_rows_to_poly_values(poseidon2_rows);
    let halfword_memory_trace = trace_rows_to_poly_values(halfword_memory_rows);
    let fullword_memory_trace = trace_rows_to_poly_values(fullword_memory_rows);
    let register_init_trace = trace_rows_to_poly_values(register_init_rows);
    let register_trace = trace_rows_to_poly_values(register_rows);
    let poseidon2_sponge_trace = trace_rows_to_poly_values(poseiden2_sponge_rows);
    [
        cpu_trace,
        rangecheck_trace,
        xor_trace,
        shift_amount_trace,
        program_trace,
        memory_trace,
        memory_init_trace,
        rangecheck_limb_trace,
        poseidon2_trace,
        poseidon2_sponge_trace,
        halfword_memory_trace,
        fullword_memory_trace,
        register_init_trace,
        register_trace,
    ]
}

#[must_use]
pub fn transpose_polys<
    F: RichField + Extendable<D> + PackedField,
    const D: usize,
    S: Stark<F, D>,
>(
    cols: Vec<PolynomialValues<F>>,
) -> Vec<Vec<F>> {
    transpose(
        &cols
            .into_iter()
            .map(|PolynomialValues { values }| values)
            .collect_vec(),
    )
    .into_iter()
    .collect_vec()
}

pub fn debug_traces<F: RichField + Extendable<D>, const D: usize>(
    traces_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    mozak_stark: &MozakStark<F, D>,
    public_inputs: &PublicInputs<F>,
) {
    let [cpu, rangecheck, xor, shift_amount, program, memory, memory_init, rangecheck_limb, poseidon2, poseidon2_sponge, halfword_memory, fullword_memory, register_init, register] =
        traces_poly_values;
    assert!(
        [
            // Program ROM
            debug_single_trace::<F, D, ProgramStark<F, D>>(&mozak_stark.program_stark, program, &[
            ],),
            // CPU
            debug_single_trace::<F, D, CpuStark<F, D>>(
                &mozak_stark.cpu_stark,
                cpu,
                public_inputs.borrow()
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
            debug_single_trace::<F, D, HalfWordMemoryStark<F, D>>(
                &mozak_stark.halfword_memory_stark,
                halfword_memory,
                &[],
            ),
            debug_single_trace::<F, D, FullWordMemoryStark<F, D>>(
                &mozak_stark.fullword_memory_stark,
                fullword_memory,
                &[],
            ),
            debug_single_trace::<F, D, RegisterInitStark<F, D>>(
                &mozak_stark.register_init_stark,
                register_init,
                &[],
            ),
            debug_single_trace::<F, D, RegisterStark<F, D>>(
                &mozak_stark.register_stark,
                register,
                &[],
            ),
            debug_single_trace::<F, D, Poseidon2_12Stark<F, D>>(
                &mozak_stark.poseidon2_stark,
                poseidon2,
                &[],
            ),
            debug_single_trace::<F, D, Poseidon2SpongeStark<F, D>>(
                &mozak_stark.poseidon2_sponge_stark,
                poseidon2_sponge,
                &[],
            ),
        ]
        .into_iter()
        .all(|x| x)
    );
}

pub fn debug_single_trace<
    F: RichField + Extendable<D>,
    const D: usize,
    S: Stark<F, D> + Display,
>(
    stark: &S,
    trace_rows: &[PolynomialValues<F>],
    public_inputs: &[F],
) -> bool {
    transpose_polys::<F, D, S>(trace_rows.to_vec())
        .iter()
        .enumerate()
        .circular_tuple_windows()
        .map(|((lv_row, lv), (nv_row, nv))| {
            let mut consumer = ConstraintConsumer::new_debug_api(lv_row == 0, nv_row == 0);
            let vars =
                StarkEvaluationFrame::from_values(lv.as_slice(), nv.as_slice(), public_inputs);
            stark.eval_packed_generic(&vars, &mut consumer);
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
