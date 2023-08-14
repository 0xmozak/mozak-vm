pub mod bitshift;
pub mod bitwise;
pub mod cpu;
pub mod instruction;
pub mod memory;
pub mod program;
pub mod rangecheck;

use itertools::Itertools;
use mozak_vm::elf::Program;
use mozak_vm::vm::ExecutionRecord;
use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;
use plonky2::util::transpose;
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

use self::bitshift::generate_shift_amount_trace;
use self::bitwise::generate_bitwise_trace;
use self::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
use self::rangecheck::generate_rangecheck_trace;
use crate::bitshift::stark::BitshiftStark;
use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::generation::program::generate_program_rom_trace;
use crate::program::stark::ProgramStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::stark::mozak_stark::{MozakStark, NUM_TABLES};
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    record: &ExecutionRecord,
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(program, &record);
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows);
    let bitwise_rows = generate_bitwise_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    let program_rows = generate_program_rom_trace(program);

    let cpu_trace = trace_to_poly_values(generate_cpu_trace_extended(cpu_rows, &program_rows));
    let rangecheck_trace = trace_to_poly_values(rangecheck_rows);
    let bitwise_trace = trace_rows_to_poly_values(bitwise_rows);
    let shift_amount_trace = trace_rows_to_poly_values(shift_amount_rows);
    let program_trace = trace_rows_to_poly_values(program_rows);
    [
        cpu_trace,
        rangecheck_trace,
        bitwise_trace,
        shift_amount_trace,
        program_trace,
    ]
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
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

#[allow(clippy::missing_panics_doc)]
pub fn debug_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    record: &ExecutionRecord,
    mozak_stark: &MozakStark<F, D>,
) where
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); BitwiseStark::<F, D>::COLUMNS]:,
    [(); BitshiftStark::<F, D>::COLUMNS]:,
    [(); ProgramStark::<F, D>::COLUMNS]:, {
    let [cpu_trace, rangecheck_trace, bitwise_trace, shift_amount_trace, program_trace]: [Vec<
        PolynomialValues<F>,
    >;
        NUM_TABLES] = generate_traces(program, record);

    assert!([
        // Program ROM
        debug_single_trace::<F, D, ProgramStark<F, D>>(
            &mozak_stark.program_stark,
            program_trace,
            "PROGRAM_ROM_STARK",
        ),
        // CPU
        debug_single_trace::<F, D, CpuStark<F, D>>(&mozak_stark.cpu_stark, cpu_trace, "CPU_STARK"),
        // Range check
        debug_single_trace::<F, D, RangeCheckStark<F, D>>(
            &mozak_stark.rangecheck_stark,
            rangecheck_trace,
            "RANGE_CHECK_STARK",
        ),
        // Bitwise
        debug_single_trace::<F, D, BitwiseStark<F, D>>(
            &mozak_stark.bitwise_stark,
            bitwise_trace,
            "BITWISE_STARK",
        ),
        // Bitshift
        debug_single_trace::<F, D, BitshiftStark<F, D>>(
            &mozak_stark.shift_amount_stark,
            shift_amount_trace,
            "BITWISE_STARK",
        ),
    ]
    .into_iter()
    .all(|x| x));
}

#[allow(clippy::missing_panics_doc)]
pub fn debug_single_trace<F: RichField + Extendable<D>, const D: usize, S: Stark<F, D>>(
    stark: &S,
    trace_rows: Vec<PolynomialValues<F>>,
    stark_name: &str,
) -> bool
where
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:, {
    transpose_polys::<F, D, S>(trace_rows)
        .iter()
        .enumerate()
        .circular_tuple_windows()
        .map(|((lv_row, lv), (nv_row, nv))| {
            let mut consumer = ConstraintConsumer::new_debug_api(lv_row == 0, nv_row == 0);
            stark.eval_packed_generic(
                StarkEvaluationVars {
                    local_values: lv,
                    next_values: nv,
                    public_inputs: &[F::ZERO; S::PUBLIC_INPUTS],
                },
                &mut consumer,
            );
            if consumer.debug_api_has_constraint_failed() {
                println!("Debug constraints for {stark_name}");
                println!("lv-row[{lv_row}] - values: {lv:?}");
                println!("nv-row[{nv_row}] - values: {nv:?}");
                false
            } else {
                true
            }
        })
        .all(|x| x)
}
