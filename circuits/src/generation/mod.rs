//! This module is responsible for populating the the Stark Tables with the
//! appropriate values based on the [`Program`] and [`ExecutionRecord`].

use std::fmt::Debug;
pub mod bitshift;
pub mod cpu;
pub mod fullword_memory;
pub mod halfword_memory;
pub mod instruction;
pub mod io_memory;
pub mod memory;
pub mod memory_zeroinit;
pub mod memoryinit;
pub mod program;
pub mod rangecheck;
pub mod rangecheck_u8;
pub mod tape_commitments;
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
use self::cpu::{generate_cpu_trace, generate_program_mult_trace};
use self::fullword_memory::generate_fullword_memory_trace;
use self::halfword_memory::generate_halfword_memory_trace;
use self::io_memory::{
    generate_call_tape_trace, generate_cast_list_commitment_tape_trace,
    generate_events_commitment_tape_trace,
};
use self::memory::generate_memory_trace;
use self::memoryinit::{
    generate_call_tape_init_trace, generate_event_tape_init_trace, generate_memory_init_trace,
    generate_private_tape_init_trace, generate_public_tape_init_trace,
};
use self::rangecheck::generate_rangecheck_trace;
use self::rangecheck_u8::generate_rangecheck_u8_trace;
use self::tape_commitments::generate_tape_commitments_trace;
use self::xor::generate_xor_trace;
use crate::columns_view::HasNamedColumns;
use crate::generation::io_memory::{
    generate_io_memory_private_trace, generate_io_memory_public_trace,
};
use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
use crate::generation::memoryinit::{
    generate_elf_memory_init_trace, generate_mozak_memory_init_trace,
};
use crate::generation::program::generate_program_rom_trace;
use crate::poseidon2::generation::generate_poseidon2_trace;
use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
use crate::register::generation::{generate_register_init_trace, generate_register_trace};
use crate::stark::mozak_stark::{
    all_starks, MozakStark, PublicInputs, TableKindArray, TableKindSetBuilder,
};
use crate::stark::utils::trace_rows_to_poly_values;

pub const MIN_TRACE_LENGTH: usize = 8;

/// Generate Constrained traces for each type of gadgets
/// Returns the polynomial encoding of each row
///
/// ## Parameters
/// `program`: A serialized ELF Program
/// `record`: Non-constrained execution trace generated by the runner
#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    record: &ExecutionRecord<F>,
) -> TableKindArray<Vec<PolynomialValues<F>>> {
    let cpu_rows = generate_cpu_trace::<F>(record);
    let xor_rows = generate_xor_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    let program_rows = generate_program_rom_trace(program);
    let program_mult_rows = generate_program_mult_trace(&cpu_rows, &program_rows);
    let elf_memory_init_rows = generate_elf_memory_init_trace(program);
    let mozak_memory_init_rows = generate_mozak_memory_init_trace(program);
    let call_tape_init_rows = generate_call_tape_init_trace(program);
    let private_tape_init_rows = generate_private_tape_init_trace(program);
    let public_tape_init_rows = generate_public_tape_init_trace(program);
    let event_tape_init_rows = generate_event_tape_init_trace(program);
    let halfword_memory_rows = generate_halfword_memory_trace(&record.executed);
    let fullword_memory_rows = generate_fullword_memory_trace(&record.executed);
    let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
    let io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
    let call_tape_rows = generate_call_tape_trace(&record.executed);
    let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
    let cast_list_commitment_tape_rows = generate_cast_list_commitment_tape_trace(&record.executed);
    let poseiden2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);
    #[allow(unused)]
    let poseidon2_output_bytes_rows = generate_poseidon2_output_bytes_trace(&poseiden2_sponge_rows);
    #[allow(unused)]
    let poseidon2_rows = generate_poseidon2_trace(&record.executed);
    let memory_rows = generate_memory_trace(
        &record.executed,
        &generate_memory_init_trace(program),
        &halfword_memory_rows,
        &fullword_memory_rows,
        &io_memory_private_rows,
        &io_memory_public_rows,
        &poseiden2_sponge_rows,
        &poseidon2_output_bytes_rows,
    );
    let memory_zeroinit_rows = generate_memory_zero_init_trace::<F>(&record.executed, program);

    let register_init_rows = generate_register_init_trace::<F>(record);
    let (register_zero_read_rows, register_zero_write_rows, register_rows) =
        generate_register_trace(
            &cpu_rows,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &call_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &register_init_rows,
        );
    // Generate rows for the looking values with their multiplicities.
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows, &register_rows);
    // Generate a trace of values containing 0..u8::MAX, with multiplicities to be
    // looked.
    let rangecheck_u8_rows = generate_rangecheck_u8_trace(&rangecheck_rows, &memory_rows);
    let tape_commitments_rows = generate_tape_commitments_trace(record);

    TableKindSetBuilder {
        cpu_stark: trace_rows_to_poly_values(cpu_rows),
        rangecheck_stark: trace_rows_to_poly_values(rangecheck_rows),
        xor_stark: trace_rows_to_poly_values(xor_rows),
        shift_amount_stark: trace_rows_to_poly_values(shift_amount_rows),
        program_stark: trace_rows_to_poly_values(program_rows),
        program_mult_stark: trace_rows_to_poly_values(program_mult_rows),
        memory_stark: trace_rows_to_poly_values(memory_rows),
        elf_memory_init_stark: trace_rows_to_poly_values(elf_memory_init_rows),
        mozak_memory_init_stark: trace_rows_to_poly_values(mozak_memory_init_rows),
        call_tape_init_stark: trace_rows_to_poly_values(call_tape_init_rows),
        private_tape_init_stark: trace_rows_to_poly_values(private_tape_init_rows),
        public_tape_init_stark: trace_rows_to_poly_values(public_tape_init_rows),
        event_tape_init_stark: trace_rows_to_poly_values(event_tape_init_rows),
        memory_zeroinit_stark: trace_rows_to_poly_values(memory_zeroinit_rows),
        rangecheck_u8_stark: trace_rows_to_poly_values(rangecheck_u8_rows),
        halfword_memory_stark: trace_rows_to_poly_values(halfword_memory_rows),
        fullword_memory_stark: trace_rows_to_poly_values(fullword_memory_rows),
        io_memory_private_stark: trace_rows_to_poly_values(io_memory_private_rows),
        io_memory_public_stark: trace_rows_to_poly_values(io_memory_public_rows),
        call_tape_stark: trace_rows_to_poly_values(call_tape_rows),
        events_commitment_tape_stark: trace_rows_to_poly_values(events_commitment_tape_rows),
        cast_list_commitment_tape_stark: trace_rows_to_poly_values(cast_list_commitment_tape_rows),
        register_init_stark: trace_rows_to_poly_values(register_init_rows),
        register_stark: trace_rows_to_poly_values(register_rows),
        register_zero_read_stark: trace_rows_to_poly_values(register_zero_read_rows),
        register_zero_write_stark: trace_rows_to_poly_values(register_zero_write_rows),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_stark: trace_rows_to_poly_values(poseidon2_rows),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_sponge_stark: trace_rows_to_poly_values(poseiden2_sponge_rows),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_output_bytes_stark: trace_rows_to_poly_values(poseidon2_output_bytes_rows),
        tape_commitments_stark: trace_rows_to_poly_values(tape_commitments_rows),
    }
    .build()
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
    traces_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    mozak_stark: &MozakStark<F, D>,
    public_inputs: &PublicInputs<F>,
) {
    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_stark: public_inputs.borrow(),
        ..Default::default()
    }
    .build();

    all_starks!(mozak_stark, |stark, kind| {
        debug_single_trace::<F, D, _>(stark, &traces_poly_values[kind], public_inputs[kind]);
    });
}

pub fn debug_single_trace<
    F: RichField + Extendable<D> + Debug,
    const D: usize,
    S: Stark<F, D> + Display + HasNamedColumns,
>(
    stark: &S,
    trace_rows: &[PolynomialValues<F>],
    public_inputs: &[F],
) where
    S::Columns: FromIterator<F> + Debug, {
    transpose_polys::<F, D, S>(trace_rows.to_vec())
        .iter()
        .enumerate()
        .circular_tuple_windows()
        .for_each(|((lv_row, lv), (nv_row, nv))| {
            let mut consumer = ConstraintConsumer::new_debug_api(lv_row == 0, nv_row == 0);
            let vars =
                StarkEvaluationFrame::from_values(lv.as_slice(), nv.as_slice(), public_inputs);
            stark.eval_packed_generic(&vars, &mut consumer);
            if consumer.debug_api_has_constraint_failed() {
                let lv: S::Columns = lv.iter().copied().collect();
                let nv: S::Columns = nv.iter().copied().collect();
                log::error!("Debug constraints for {stark}");
                log::error!("lv-row[{lv_row}] - values: {lv:?}");
                log::error!("nv-row[{nv_row}] - values: {nv:?}");
            }
            assert!(!consumer.debug_api_has_constraint_failed());
        });
}
