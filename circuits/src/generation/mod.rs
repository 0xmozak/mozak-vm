pub mod bitshift;
pub mod bitwise;
pub mod cpu;
pub mod instruction;
pub mod memory;
pub mod rangecheck;

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
use crate::cpu::stark::CpuStark;
use crate::generation::program::generate_program_rom_trace;
use crate::rangecheck::stark::RangeCheckStark;
use crate::stark::mozak_stark::{MozakStark, NUM_TABLES, NUM_TABLES};
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    step_rows: &[Row],
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(program, step_rows);
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows);
    let bitwise_rows = generate_bitwise_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);

    let cpu_trace = trace_rows_to_poly_values(cpu_rows);
    let rangecheck_trace = trace_to_poly_values(rangecheck_rows);
    let bitwise_trace = trace_rows_to_poly_values(bitwise_rows);
    let shift_amount_trace = trace_rows_to_poly_values(shift_amount_rows);
    let program_trace = trace_rows_to_poly_values(generate_program_rom_trace(program, &cpu_rows));

    let cpu_trace = trace_to_poly_values(generate_cpu_trace_extended(cpu_rows));
    [
        cpu_trace,
        rangecheck_trace,
        bitwise_trace,
        shift_amount_trace,
        program_trace,
    ]
}
#[allow(clippy::needless_for_each)]
#[allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]
pub fn debug_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    step_rows: &[Row],
    mozak_stark: &MozakStark<F, D>,
) where
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); BitwiseStark::<F, D>::COLUMNS]:,
    [(); BitshiftStark::<F, D>::COLUMNS]:, {
    let mut rc = true;
    // [0] - CPU
    let cpu_rows = generate_cpu_trace::<F>(program, step_rows);
    let mut generic_cpu_rows: Vec<Vec<F>> = vec![];
    cpu_rows.iter().for_each(|row| {
        generic_cpu_rows.push(row.into_iter().as_slice().try_into().unwrap());
    });
    rc &= debug_single_trace::<F, D, CpuStark<F, D>>(
        &mozak_stark.cpu_stark,
        &generic_cpu_rows,
        "CPU_STARK",
        false,
    );
    // [1] - RC
    let rc_rows = generate_rangecheck_trace::<F>(&cpu_rows);
    rc &= debug_single_trace::<F, D, RangeCheckStark<F, D>>(
        &mozak_stark.rangecheck_stark,
        &transpose(&rc_rows),
        "RANGE_CHECK_STARK",
        true,
    );
    // [2] - BW
    let bitwise_rows = generate_bitwise_trace(&cpu_rows);
    let mut generic_bw_rows: Vec<Vec<F>> = vec![];
    bitwise_rows.iter().for_each(|row| {
        generic_bw_rows.push(row.into_iter().as_slice().try_into().unwrap());
    });
    rc &= debug_single_trace::<F, D, BitwiseStark<F, D>>(
        &mozak_stark.bitwise_stark,
        &generic_bw_rows,
        "BITWISE_STARK",
        false,
    );

    // [3] - BS
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    let mut generic_bitwise_rows: Vec<Vec<F>> = vec![];
    shift_amount_rows.iter().for_each(|row| {
        generic_bitwise_rows.push(row.into_iter().as_slice().try_into().unwrap());
    });
    rc &= debug_single_trace::<F, D, BitshiftStark<F, D>>(
        &mozak_stark.shift_amount_stark,
        &generic_bitwise_rows,
        "BITWISE_STARK",
        false,
    );
    assert!(rc);
}
#[allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]
pub fn debug_single_trace<F: RichField + Extendable<D>, const D: usize, S: Stark<F, D>>(
    s: &S,
    trace_rows: &Vec<Vec<F>>,
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
        trace_rows[lv_row].iter().for_each(|e| {
            lv.push(*e);
        });

        let mut nv: Vec<_> = vec![];
        trace_rows[nv_row].iter().for_each(|e| {
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
                trace_rows[0].iter().for_each(|e| nv.push(*e));
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
            println!("lv-row[{lv_row}] - values: {:?}", lv);
            println!("nv-row[{nv_row}] - values: {:?}", nv);
            consumer.debug_api_reset_failed_constraint();
            rc = false;
        }
    }
    rc
}
