use std::collections::BTreeMap;
use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::memory::columns::Memory;
use crate::ops::add::columns::Add;
use crate::ops::lw::columns::LoadWord;
use crate::ops::sw::columns::StoreWord;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::register::general::columns::Register;
use crate::stark::mozak_stark::{Lookups, RangecheckTable, Table, TableKind};
use crate::utils::pad_trace_with_default;

/// Converts a u32 into 4 u8 limbs represented in [`RichField`].
#[must_use]
pub fn limbs_from_u32<F: RichField>(value: u32) -> [F; 4] {
    value.to_le_bytes().map(|v| F::from_canonical_u8(v))
}

/// extract the values to be rangechecked.
/// multiplicity is assumed to be 0 or 1 since we apply this only for cpu and
/// memory traces, hence ignored
pub fn extract<'a, F: RichField, V>(trace: &[V], looking_table: &Table) -> Vec<F>
where
    V: Index<usize, Output = F> + 'a, {
    if let [column] = &looking_table.columns[..] {
        trace
            .iter()
            .circular_tuple_windows()
            .filter(|&(prev_row, row)| looking_table.filter_column.eval(prev_row, row).is_one())
            .map(|(prev_row, row)| column.eval(prev_row, row))
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
    add_trace: &[Add<F>],
    store_word_trace: &[StoreWord<F>],
    load_word_trace: &[LoadWord<F>],
    memory_trace: &[Memory<F>],
    register_trace: &[Register<F>],
) -> Vec<RangeCheckColumnsView<F>> {
    let mut multiplicities: BTreeMap<u32, u64> = BTreeMap::new();

    RangecheckTable::lookups()
        .looking_tables
        .into_iter()
        .for_each(|looking_table| {
            match looking_table.kind {
                TableKind::Cpu => extract(cpu_trace, &looking_table),
                TableKind::Memory => extract(memory_trace, &looking_table),
                TableKind::Register => extract(register_trace, &looking_table),
                TableKind::Add => extract(add_trace, &looking_table),
                TableKind::StoreWord => extract(store_word_trace, &looking_table),
                TableKind::LoadWord => extract(load_word_trace, &looking_table),
                other => unimplemented!("Can't range check {other:#?} tables"),
            }
            .into_iter()
            .for_each(|v| {
                let val = u32::try_from(v.to_canonical_u64())
                    .expect("casting value to u32 should succeed");

                *multiplicities.entry(val).or_default() += 1;
            });
        });
    let mut trace = Vec::with_capacity(multiplicities.len());
    for (value, multiplicity) in multiplicities {
        trace.push(RangeCheckColumnsView {
            multiplicity: F::from_canonical_u64(multiplicity),
            limbs: limbs_from_u32(value),
        });
    }

    pad_trace_with_default(trace)
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::util::execute_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
        generate_io_transcript_trace,
    };
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::poseidon2_output_bytes::generate_poseidon2_output_bytes_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::generation::MIN_TRACE_LENGTH;
    use crate::ops::{self, blt_taken};
    use crate::register::generation::{generate_register_init_trace, generate_register_trace};

    #[test]
    fn test_generate_trace() {
        type F = GoldilocksField;
        let (program, record) = execute_code(
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
        let add_rows = ops::add::generate(&record);
        let store_word_rows = ops::sw::generate(&record);
        let load_word_rows = ops::lw::generate(&record);
        let blt_rows = blt_taken::generate(&record);
        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
        let io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
        let io_transcript_rows = generate_io_transcript_trace(&record.executed);
        let poseidon2_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_trace);
        let memory_rows = generate_memory_trace::<F>(
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &poseidon2_trace,
            &poseidon2_output_bytes,
        );
        let register_init = generate_register_init_trace(&record);
        let (_, _, register_rows) = generate_register_trace(
            &cpu_rows,
            &add_rows,
            &store_word_rows,
            &load_word_rows,
            &blt_rows,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &io_transcript_rows,
            &register_init,
        );
        let trace = generate_rangecheck_trace::<F>(
            &cpu_rows,
            &add_rows,
            &store_word_rows,
            &load_word_rows,
            &memory_rows,
            &register_rows,
        );
        assert_eq!(
            trace.len(),
            MIN_TRACE_LENGTH,
            "Unexpected trace len {}",
            trace.len()
        );
        for (i, row) in trace.iter().enumerate() {
            match i {
                0 => assert_eq!(row.multiplicity, F::from_canonical_u8(2)),
                1 => assert_eq!(row.multiplicity, F::from_canonical_u8(1)),
                _ => {}
            }
        }
    }
}
