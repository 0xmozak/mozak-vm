//! This module is responsible for populating the the Stark Tables with the
//! appropriate values based on the [`Program`] and [`ExecutionRecord`].

use std::borrow::Borrow;
use std::fmt::{Debug, Display};

use expr::ExprBuilder;
use itertools::{izip, Itertools};
use log::debug;
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;
use plonky2::util::timing::TimingTree;
use plonky2::util::transpose;
use starky::stark::Stark;

use crate::bitshift::generation::generate_shift_amount_trace;
use crate::cpu::generation::{generate_cpu_trace, generate_program_mult_trace};
use crate::cpu_skeleton::generation::generate_cpu_skeleton_trace;
use crate::expr::{build_debug, GenerateConstraints, ConstraintType};
use crate::memory::generation::generate_memory_trace;
use crate::memory_fullword::generation::generate_fullword_memory_trace;
use crate::memory_halfword::generation::generate_halfword_memory_trace;
use crate::memory_zeroinit::generation::generate_memory_zero_init_trace;
use crate::memoryinit::generation::{generate_elf_memory_init_trace, generate_memory_init_trace};
use crate::ops;
use crate::poseidon2::generation::generate_poseidon2_trace;
use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
use crate::program::generation::generate_program_rom_trace;
use crate::rangecheck::generation::generate_rangecheck_trace;
use crate::rangecheck_u8::generation::generate_rangecheck_u8_trace;
use crate::register::generation::{generate_register_init_trace, generate_register_trace};
use crate::stark::mozak_stark::{
    all_starks, MozakStark, PublicInputs, TableKindArray, TableKindSetBuilder,
};
use crate::stark::utils::trace_rows_to_poly_values;
use crate::storage_device::generation::{
    generate_call_tape_trace, generate_cast_list_commitment_tape_trace, generate_event_tape_trace,
    generate_events_commitment_tape_trace, generate_private_tape_trace, generate_public_tape_trace,
    generate_self_prog_id_tape_trace,
};
use crate::tape_commitments::generation::generate_tape_commitments_trace;
use crate::xor::generation::generate_xor_trace;

pub const MIN_TRACE_LENGTH: usize = 8;

/// Generate Constrained traces for each type of gadgets
/// Returns the polynomial encoding of each row
///
/// ## Parameters
/// `program`: A serialized ELF Program
/// `record`: Non-constrained execution trace generated by the runner
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    record: &ExecutionRecord<F>,
    _timing: &mut TimingTree,
) -> TableKindArray<Vec<PolynomialValues<F>>> {
    debug!("Starting Trace Generation");
    let cpu_rows = generate_cpu_trace::<F>(record);
    let skeleton_rows = generate_cpu_skeleton_trace(record);
    let add_rows = ops::add::generate(record);
    let blt_taken_rows = ops::blt_taken::generate(record);
    let xor_rows = generate_xor_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);
    let program_rows = generate_program_rom_trace(program);
    let program_mult_rows = generate_program_mult_trace(&skeleton_rows, &program_rows);

    let memory_init = generate_memory_init_trace(program);
    let elf_memory_init_rows = generate_elf_memory_init_trace(program);

    let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, program);

    let halfword_memory_rows = generate_halfword_memory_trace(&record.executed);
    let fullword_memory_rows = generate_fullword_memory_trace(&record.executed);
    let private_tape_rows = generate_private_tape_trace(&record.executed);
    let public_tape_rows = generate_public_tape_trace(&record.executed);
    let call_tape_rows = generate_call_tape_trace(&record.executed);
    let event_tape_rows = generate_event_tape_trace(&record.executed);
    let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
    let cast_list_commitment_tape_rows = generate_cast_list_commitment_tape_trace(&record.executed);
    let self_prog_id_tape_rows = generate_self_prog_id_tape_trace(&record.executed);
    let poseiden2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);
    let poseidon2_output_bytes_rows = generate_poseidon2_output_bytes_trace(&poseiden2_sponge_rows);
    let poseidon2_rows = generate_poseidon2_trace(&record.executed);

    let memory_rows = generate_memory_trace(
        &record.executed,
        &memory_init,
        &memory_zeroinit_rows,
        &halfword_memory_rows,
        &fullword_memory_rows,
        &private_tape_rows,
        &public_tape_rows,
        &call_tape_rows,
        &event_tape_rows,
        &events_commitment_tape_rows,
        &cast_list_commitment_tape_rows,
        &self_prog_id_tape_rows,
        &poseiden2_sponge_rows,
        &poseidon2_output_bytes_rows,
    );

    let register_init_rows = generate_register_init_trace::<F>(record);
    let (register_zero_read_rows, register_zero_write_rows, register_rows) =
        generate_register_trace(
            &cpu_rows,
            &add_rows,
            &blt_taken_rows,
            &poseiden2_sponge_rows,
            &private_tape_rows,
            &public_tape_rows,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &self_prog_id_tape_rows,
            &register_init_rows,
        );
    // Generate rows for the looking values with their multiplicities.
    let rangecheck_rows = generate_rangecheck_trace::<F>(
        &cpu_rows,
        &add_rows,
        &blt_taken_rows,
        &memory_rows,
        &register_rows,
    );
    // Generate a trace of values containing 0..u8::MAX, with multiplicities to be
    // looked.
    let rangecheck_u8_rows = generate_rangecheck_u8_trace(&rangecheck_rows, &memory_rows);
    let add_trace = ops::add::generate(record);
    let blt_trace = ops::blt_taken::generate(record);
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
        memory_zeroinit_stark: trace_rows_to_poly_values(memory_zeroinit_rows),
        rangecheck_u8_stark: trace_rows_to_poly_values(rangecheck_u8_rows),
        halfword_memory_stark: trace_rows_to_poly_values(halfword_memory_rows),
        fullword_memory_stark: trace_rows_to_poly_values(fullword_memory_rows),
        private_tape_stark: trace_rows_to_poly_values(private_tape_rows),
        public_tape_stark: trace_rows_to_poly_values(public_tape_rows),
        call_tape_stark: trace_rows_to_poly_values(call_tape_rows),
        event_tape_stark: trace_rows_to_poly_values(event_tape_rows),
        events_commitment_tape_stark: trace_rows_to_poly_values(events_commitment_tape_rows),
        cast_list_commitment_tape_stark: trace_rows_to_poly_values(cast_list_commitment_tape_rows),
        self_prog_id_tape_stark: trace_rows_to_poly_values(self_prog_id_tape_rows),
        register_init_stark: trace_rows_to_poly_values(register_init_rows),
        register_stark: trace_rows_to_poly_values(register_rows),
        register_zero_read_stark: trace_rows_to_poly_values(register_zero_read_rows),
        register_zero_write_stark: trace_rows_to_poly_values(register_zero_write_rows),
        poseidon2_stark: trace_rows_to_poly_values(poseidon2_rows),
        poseidon2_sponge_stark: trace_rows_to_poly_values(poseiden2_sponge_rows),
        poseidon2_output_bytes_stark: trace_rows_to_poly_values(poseidon2_output_bytes_rows),
        cpu_skeleton_stark: trace_rows_to_poly_values(skeleton_rows),
        add_stark: trace_rows_to_poly_values(add_trace),
        blt_taken_stark: trace_rows_to_poly_values(blt_trace),
        tape_commitments_stark: trace_rows_to_poly_values(tape_commitments_rows),
    }
    .build()
}

pub fn ascending_sum<F: RichField, I: IntoIterator<Item = F>>(cs: I) -> F {
    izip![(0..).map(F::from_canonical_u64), cs]
        .map(|(i, x)| i * x)
        .sum()
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
        cpu_skeleton_stark: public_inputs.borrow(),
        ..Default::default()
    }
    .build();

    all_starks!(mozak_stark, |stark, kind| {
        debug_single_trace::<F, D, _>(stark, &traces_poly_values[kind], public_inputs[kind]);
    });
}

pub fn debug_single_trace<
    'a,
    F: RichField + Extendable<D> + Debug,
    const D: usize,
    S
>(
    stark: &'a S,
    trace_rows: &'a [PolynomialValues<F>],
    public_inputs: &'a [F],
) where
    for <'b> S: Stark<F, D> + Display + GenerateConstraints<'b, F>,
    {
    type View<'a, S, F> = <S as GenerateConstraints<'a, F>>::View<F>;
    transpose_polys::<F, D, S>(trace_rows.to_vec())
        .iter()
        .enumerate()
        .circular_tuple_windows()
        .for_each(|((lv_row, lv), (nv_row, nv))| {
            let expr_builder = ExprBuilder::default();
            let vars = &expr_builder.to_typed_starkframe_(lv.as_slice(), nv.as_slice(), public_inputs, S::COLUMNS, S::PUBLIC_INPUTS);
            let constraints = S::generate_constraints(vars);
            let evaluated = build_debug(constraints);

            // Filter out only applicable constraints
            let is_first_row = lv_row == 0;
            let is_last_row = nv_row == 0;
            let applicable = evaluated.into_iter()
                .filter( |c|
                    match c.constraint_type {
                        ConstraintType::FirstRow => is_first_row,
                        ConstraintType::Always => true,
                        ConstraintType::Transition => !is_last_row,
                        ConstraintType::LastRow => is_last_row,
                    }
                );

            // Get failed constraints
            let failed: Vec<_>=
                applicable
                    .filter(|c| !c.term.is_zeros())
                    .collect();

            let any_failed = !failed.is_empty();

            if any_failed {
                for c in failed {
                    log::error!("debug_single_trace :: non-zero constraint at {} = {}", c.location, c.term);
                }

                let lv: View<S, F> = lv.iter().copied().collect();
                let nv: View<S, F> = nv.iter().copied().collect();
                log::error!("Debug constraints for {stark}");
                log::error!("lv-row[{lv_row}] - values: {lv:?}");
                log::error!("nv-row[{nv_row}] - values: {nv:?}");
            }

            assert!(!any_failed);
        });
}
