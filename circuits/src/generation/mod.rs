pub mod bitshift;
pub mod bitwise;
pub mod cpu;
pub mod instruction;
pub mod memory;
pub mod program;
pub mod rangecheck;
use std::borrow::Borrow;

use itertools::Itertools;
use mozak_vm::elf::Program;
use mozak_vm::vm::Row;
use plonky2::field::extension::Extendable;
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
use crate::cpu::columns::CpuColumnsExtended;
use crate::cpu::stark::CpuStark;
use crate::generation::program::generate_program_rom_trace;
use crate::program::stark::ProgramStark;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck::stark::RangeCheckStark;
use crate::stark::mozak_stark::{MozakStark, NUM_TABLES};
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values, transpose_trace};

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    step_rows: &[Row],
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(program, step_rows);
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

#[allow(clippy::missing_panics_doc)]
pub fn debug_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    step_rows: &[Row],
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
        NUM_TABLES] = generate_traces(program, step_rows);
    let mut rc = true;

    // [0] - PR
    let program_rom_rows = generate_program_rom_trace(program);

    rc &= debug_single_trace::<F, D, ProgramStark<F, D>>(
        &mozak_stark.program_stark,
        &program_rom_rows,
        "PROGRAM_ROM_STARK",
        false,
    );

    // [1] - CPU
    let cpu_rows = generate_cpu_trace::<F>(program, step_rows);
    let cpu_rows_extended = generate_cpu_trace_extended(cpu_rows.clone(), &program_rom_rows);
    let generic_cpu_rows = transpose_trace(cpu_rows_extended.into_iter().collect_vec());
    let generic_cpu_rows = generic_cpu_rows
        .into_iter()
        .map(|row| row.into_iter().collect::<CpuColumnsExtended<F>>())
        .collect_vec();

    rc &= debug_single_trace::<F, D, CpuStark<F, D>>(
        &mozak_stark.cpu_stark,
        &generic_cpu_rows,
        "CPU_STARK",
        false,
    );

    // [2] - RC
    let rc_rows = generate_rangecheck_trace::<F>(&cpu_rows);
    let rc_rows = transpose(&rc_rows);
    let rc_rows = rc_rows
        .iter()
        .map(|row| row.iter().copied().collect::<RangeCheckColumnsView<F>>())
        .collect_vec();
    // let rc_rows = rc_rows.iter().map(Borrow::borrow).collect_vec();
    rc &= debug_single_trace::<F, D, RangeCheckStark<F, D>>(
        &mozak_stark.rangecheck_stark,
        &rc_rows,
        "RANGE_CHECK_STARK",
        true,
    );
    // [3] - BW
    let bitwise_rows = generate_bitwise_trace(&cpu_rows);
    rc &= debug_single_trace::<F, D, BitwiseStark<F, D>>(
        &mozak_stark.bitwise_stark,
        &bitwise_rows,
        "BITWISE_STARK",
        false,
    );

    // [4] - BS
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);

    rc &= debug_single_trace::<F, D, BitshiftStark<F, D>>(
        &mozak_stark.shift_amount_stark,
        &shift_amount_rows,
        "BITWISE_STARK",
        false,
    );

    assert!(rc);
}

#[allow(clippy::missing_panics_doc)]
pub fn debug_single_trace<F: RichField + Extendable<D>, const D: usize, S: Stark<F, D>>(
    s: &S,
    trace_rows: &Vec<impl Borrow<[F; S::COLUMNS]>>,
    stark_name: &str,
    is_range_check: bool,
) -> bool
where
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:, {
    let mut rc = true;
    let mut consumer = ConstraintConsumer::new_debug_api();
    for nv_row in 1..trace_rows.len() {
        let lv_row = nv_row - 1;
        let mut lv: Vec<_> = vec![];
        trace_rows[lv_row].borrow().iter().for_each(|e| {
            lv.push(*e);
        });

        let mut nv: Vec<_> = vec![];
        trace_rows[nv_row].borrow().iter().for_each(|e| {
            nv.push(*e);
        });

        if nv_row == 1 {
            consumer.debug_api_activate_first_row();
        } else if nv_row == trace_rows.len() - 1 {
            consumer.debug_api_activate_last_row();
            lv = nv.clone();
            // NOTE: this is the place that differs from other traces, please refer to
            // lookup.rs for more info
            if is_range_check {
                nv.clear();
                trace_rows[0].borrow().iter().for_each(|e| nv.push(*e));
            }
        } else {
            consumer.debug_api_activate_transition();
        }

        s.eval_packed_generic(
            StarkEvaluationVars {
                local_values: lv.as_slice().try_into().unwrap(),
                next_values: nv.as_slice().try_into().unwrap(),
                public_inputs: &[F::ZERO; S::PUBLIC_INPUTS],
            },
            &mut consumer,
        );
        if consumer.debug_api_is_constraint_failed() {
            println!("Debug constraints for {stark_name}");
            println!("lv-row[{lv_row}] - values: {lv:?}");
            println!("nv-row[{nv_row}] - values: {nv:?}");
            consumer.debug_api_reset_failed_constraint();
            rc = false;
        }
    }
    rc
}
