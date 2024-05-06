use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::memory::columns::Memory;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::register::general::columns::Register;
use crate::stark::mozak_stark::{Lookups, RangecheckTable, Table, TableKind};
use crate::utils::pad_trace_with_default;

/// Converts a u32 into 4 u8 limbs represented in [`RichField`].
#[must_use]
pub fn limbs_from_u32<F: RichField>(value: u32) -> [F; 4] {
    value.to_le_bytes().map(F::from_canonical_u8)
}

/// extract the values with multiplicities
pub fn extract_with_mul<F: RichField, Row>(trace: &[Row], looking_table: &Table) -> Vec<(F, F)>
where
    Row: Index<usize, Output = F>, {
    if let [column] = &looking_table.columns[..] {
        trace
            .iter()
            .circular_tuple_windows()
            .filter_map(|(prev_row, row)| {
                let mult = looking_table.filter_column.eval(prev_row, row);
                mult.is_nonzero().then_some((
                    column.eval(prev_row, row).to_canonical(),
                    looking_table.filter_column.eval(prev_row, row),
                ))
            })
            .collect()
    } else {
        panic!("Can only range check single values, not tuples.")
    }
}

/// Generates a trace table for range checks, used in building a
/// `RangeCheckStark` proof.
///
/// # Panics
///
/// Panics if:
/// 1. conversion of u32 values to u8 limbs fails,
/// 2. trace width does not match the number of columns,
/// 3. attempting to range check tuples instead of single values.
#[must_use]
pub(crate) fn generate_rangecheck_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    memory_trace: &[Memory<F>],
    register_trace: &[Register<F>],
) -> Vec<RangeCheckColumnsView<F>> {
    pad_trace_with_default(
        RangecheckTable::lookups()
            .looking_tables
            .into_iter()
            .flat_map(|looking_table| {
                match looking_table.kind {
                    TableKind::Cpu => extract_with_mul(cpu_trace, &looking_table),
                    TableKind::Memory => extract_with_mul(memory_trace, &looking_table),
                    TableKind::Register => extract_with_mul(register_trace, &looking_table),
                    // We are trying to build the RangeCheck table, so we have to ignore it here.
                    TableKind::RangeCheck => vec![],
                    other => unimplemented!("Can't range check {other:#?} tables"),
                }
            })
            .into_group_map()
            .into_iter()
            // Sorting just for determinism:
            .sorted_by_key(|(v, _)| v.to_noncanonical_u64())
            .map(|(v, multiplicity)| RangeCheckColumnsView {
                multiplicity: multiplicity.into_iter().sum(),
                limbs: limbs_from_u32(v.to_noncanonical_u64().try_into().unwrap_or_else(|_| {
                    panic!(
                        "We can only rangecheck values that actually fit in u32, but got: {v:#x?}"
                    )
                })),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::storage_device::{
        generate_call_tape_trace, generate_cast_list_commitment_tape_trace,
        generate_event_tape_trace, generate_events_commitment_tape_trace,
        generate_io_memory_private_trace, generate_io_memory_public_trace,
    };
    use crate::generation::MIN_TRACE_LENGTH;
    use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::register::generation::{generate_register_init_trace, generate_register_trace};

    #[test]
    fn test_generate_trace() {
        type F = GoldilocksField;
        let (program, record) = code::execute(
            [Instruction {
                op: Op::SB,
                args: Args {
                    rs1: 1,
                    imm: u32::MAX,
                    ..Args::default()
                },
            }],
            // Use values that would become limbs later
            &[],
            &[(1, u32::MAX)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&record);

        let memory_init = generate_memory_init_trace(&program);
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, &program);

        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
        let io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
        let call_tape_rows = generate_call_tape_trace(&record.executed);
        let event_tape_rows = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&record.executed);
        let poseidon2_sponge_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_sponge_trace);
        let memory_rows = generate_memory_trace::<F>(
            &record.executed,
            &memory_init,
            &memory_zeroinit_rows,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &poseidon2_sponge_trace,
            &poseidon2_output_bytes,
        );
        let register_init = generate_register_init_trace(&record);
        let (_, _, register_rows) = generate_register_trace(
            &cpu_rows,
            &poseidon2_sponge_trace,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &register_init,
        );
        let trace = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows, &register_rows);
        assert_eq!(
            trace.len(),
            MIN_TRACE_LENGTH,
            "Unexpected trace len {}",
            trace.len()
        );
        for (i, row) in trace.iter().enumerate() {
            match i {
                0 => assert_eq!(row.multiplicity, F::from_canonical_u8(7)),
                1 => assert_eq!(row.multiplicity, F::from_canonical_u8(2)),
                _ => {}
            }
        }
    }
}
