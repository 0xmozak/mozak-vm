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
pub mod poseidon2;
pub mod poseidon2_output_bytes;
pub mod poseidon2_sponge;
pub mod program;
pub mod rangecheck;
pub mod rangecheck_u8;
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
use self::io_memory::generate_io_transcript_trace;
use self::memory::generate_memory_trace;
use self::memoryinit::generate_memory_init_trace;
use self::poseidon2_output_bytes::generate_poseidon2_output_bytes_trace;
use self::poseidon2_sponge::generate_poseidon2_sponge_trace;
use self::rangecheck::generate_rangecheck_trace;
use self::rangecheck_u8::generate_rangecheck_u8_trace;
use self::register::generate_register_trace;
use self::registerinit::generate_register_init_trace;
use self::xor::generate_xor_trace;
use crate::columns_view::HasNamedColumns;
use crate::generation::cpu::generate_permuted_inst_trace;
use crate::generation::io_memory::{
    generate_io_memory_private_trace, generate_io_memory_public_trace,
};
use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
use crate::generation::poseidon2::generate_poseidon2_trace;
use crate::generation::program::generate_program_rom_trace;
use crate::stark::mozak_stark::{
    all_starks, MozakStark, PublicInputs, TableKindArray, TableKindSetBuilder,
};
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};
#[cfg(feature = "enable_batch_fri")]
use crate::utils::pad_trace_with_default_to_len;
#[cfg(feature = "enable_batch_fri")]
use crate::utils::pad_trace_with_last_to_len;

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
    let mut cpu_rows = generate_cpu_trace::<F>(record);
    #[allow(unused_mut)]
    let mut register_rows = generate_register_trace::<F>(record);
    #[allow(unused_mut)]
    let mut xor_rows = generate_xor_trace(&cpu_rows);
    #[allow(unused_mut)]
    let mut shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    #[allow(unused_mut)]
    let mut program_rows = generate_program_rom_trace(program);
    #[allow(unused_mut)]
    let mut memory_init_rows = generate_memory_init_trace(program);
    #[allow(unused_mut)]
    let mut halfword_memory_rows = generate_halfword_memory_trace(&record.executed);
    #[allow(unused_mut)]
    let mut fullword_memory_rows = generate_fullword_memory_trace(&record.executed);
    #[allow(unused_mut)]
    let mut io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
    #[allow(unused_mut)]
    let mut io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
    #[allow(unused_mut)]
    let mut io_transcript_rows = generate_io_transcript_trace(&record.executed);
    #[allow(unused_mut)]
    let mut poseiden2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);
    #[allow(unused_mut)]
    #[allow(unused)]
    let mut poseidon2_output_bytes_rows =
        generate_poseidon2_output_bytes_trace(&poseiden2_sponge_rows);
    #[allow(unused_mut)]
    #[allow(unused)]
    let mut poseidon2_rows = generate_poseidon2_trace(&record.executed);
    #[allow(unused_mut)]
    let mut memory_rows = generate_memory_trace(
        &record.executed,
        &memory_init_rows,
        &halfword_memory_rows,
        &fullword_memory_rows,
        &io_memory_private_rows,
        &io_memory_public_rows,
        &poseiden2_sponge_rows,
        &poseidon2_output_bytes_rows,
    );
    #[allow(unused_mut)]
    let mut memory_zeroinit_rows =
        generate_memory_zero_init_trace::<F>(&memory_init_rows, &record.executed);

    #[allow(unused_mut)]
    let mut rangecheck_rows =
        generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows, &register_rows);
    #[allow(unused_mut)]
    let mut rangecheck_u8_rows = generate_rangecheck_u8_trace(&rangecheck_rows, &memory_rows);
    #[allow(unused_mut)]
    #[allow(unused)]
    let mut register_init_rows = generate_register_init_trace::<F>();
    #[allow(unused_mut)]
    #[allow(unused)]
    let mut register_rows = generate_register_trace::<F>(record);
    #[allow(unused_mut)]
    let mut cpu_permuted_inst_rows = generate_permuted_inst_trace(&mut cpu_rows, &program_rows);

    #[cfg(feature = "enable_batch_fri")]
    let lengths = [
        cpu_rows.len(),
        xor_rows.len(),
        shift_amount_rows.len(),
        program_rows.len(),
        memory_init_rows.len(),
        halfword_memory_rows.len(),
        fullword_memory_rows.len(),
        io_memory_private_rows.len(),
        io_memory_public_rows.len(),
        #[cfg(feature = "enable_poseidon_starks")]
        poseiden2_sponge_rows.len(),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_output_bytes_rows.len(),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_rows.len(),
        memory_rows.len(),
        memory_zeroinit_rows.len(),
        rangecheck_rows.len(),
        rangecheck_u8_rows.len(),
        #[cfg(feature = "enable_register_starks")]
        register_init_rows.len(),
        #[cfg(feature = "enable_register_starks")]
        register_rows.len(),
        cpu_permuted_inst_rows.len(),
    ];

    #[cfg(feature = "enable_batch_fri")]
    let len = *lengths.iter().max().unwrap_or(&0);

    // TODO: carefully review the padding logic
    #[cfg(feature = "enable_batch_fri")]
    {
        cpu_rows = pad_trace_with_last_to_len(cpu_rows, len);
        xor_rows = pad_trace_with_last_to_len(xor_rows, len);
        shift_amount_rows = pad_trace_with_last_to_len(shift_amount_rows, len);
        for row in 32..shift_amount_rows.len() {
            shift_amount_rows[row].multiplicity = F::ZERO;
        }
        program_rows = pad_trace_with_default_to_len(program_rows, len);
        memory_init_rows = pad_trace_with_last_to_len(memory_init_rows, len);
        halfword_memory_rows = pad_trace_with_last_to_len(halfword_memory_rows, len);
        fullword_memory_rows = pad_trace_with_last_to_len(fullword_memory_rows, len);
        io_memory_private_rows = pad_trace_with_last_to_len(io_memory_private_rows, len);
        io_memory_public_rows = pad_trace_with_last_to_len(io_memory_public_rows, len);
        io_transcript_rows = pad_trace_with_last_to_len(io_transcript_rows, len);

        #[cfg(feature = "enable_poseidon_starks")]
        {
            poseiden2_sponge_rows = pad_trace_with_last_to_len(poseiden2_sponge_rows, len);
            poseidon2_output_bytes_rows =
                pad_trace_with_last_to_len(poseidon2_output_bytes_rows, len);
            poseidon2_rows = pad_trace_with_last_to_len(poseidon2_rows, len);
        }

        memory_rows = pad_trace_with_last_to_len(memory_rows, len);
        memory_zeroinit_rows = pad_trace_with_last_to_len(memory_zeroinit_rows, len);
        rangecheck_rows = pad_trace_with_default_to_len(rangecheck_rows, len);
        rangecheck_u8_rows = pad_trace_with_last_to_len(rangecheck_u8_rows, len);
        for row in 256..rangecheck_u8_rows.len() {
            rangecheck_u8_rows[row].multiplicity = F::ZERO;
        }

        #[cfg(feature = "enable_register_starks")]
        {
            register_init_rows = pad_trace_with_last_to_len(register_init_rows, len);
            register_rows = pad_trace_with_last_to_len(register_rows, len);
        }

        cpu_permuted_inst_rows = pad_trace_with_last_to_len(cpu_permuted_inst_rows, len);
    }

    TableKindSetBuilder {
        cpu_stark: trace_to_poly_values(generate_cpu_trace_extended(
            cpu_rows,
            cpu_permuted_inst_rows,
        )),
        rangecheck_stark: trace_rows_to_poly_values(rangecheck_rows),
        xor_stark: trace_rows_to_poly_values(xor_rows),
        shift_amount_stark: trace_rows_to_poly_values(shift_amount_rows),
        program_stark: trace_rows_to_poly_values(program_rows),
        memory_stark: trace_rows_to_poly_values(memory_rows),
        memory_init_stark: trace_rows_to_poly_values(memory_init_rows),
        memory_zeroinit_stark: trace_rows_to_poly_values(memory_zeroinit_rows),
        rangecheck_u8_stark: trace_rows_to_poly_values(rangecheck_u8_rows),
        halfword_memory_stark: trace_rows_to_poly_values(halfword_memory_rows),
        fullword_memory_stark: trace_rows_to_poly_values(fullword_memory_rows),
        io_memory_private_stark: trace_rows_to_poly_values(io_memory_private_rows),
        io_memory_public_stark: trace_rows_to_poly_values(io_memory_public_rows),
        io_transcript_stark: trace_rows_to_poly_values(io_transcript_rows),
        #[cfg(feature = "enable_register_starks")]
        register_init_stark: trace_rows_to_poly_values(register_init_rows),
        #[cfg(feature = "enable_register_starks")]
        register_stark: trace_rows_to_poly_values(register_rows),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_stark: trace_rows_to_poly_values(poseidon2_rows),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_sponge_stark: trace_rows_to_poly_values(poseiden2_sponge_rows),
        #[cfg(feature = "enable_poseidon_starks")]
        poseidon2_output_bytes_stark: trace_rows_to_poly_values(poseidon2_output_bytes_rows),
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
