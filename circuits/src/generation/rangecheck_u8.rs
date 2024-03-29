use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::memory::columns::Memory;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_u8::columns::RangeCheckU8;
use crate::stark::mozak_stark::{Lookups, RangeCheckU8LookupTable, Table, TableKind};

/// extract the values with multiplicity nonzero
pub fn extract_with_mul<F: RichField, V>(trace: &[V], looking_table: &Table) -> Vec<(F, F)>
where
    V: Index<usize, Output = F>, {
    if let [column] = &looking_table.columns[..] {
        trace
            .iter()
            .circular_tuple_windows()
            .map(|(prev_row, row)| {
                (
                    looking_table.filter_column.eval(prev_row, row),
                    column.eval(prev_row, row),
                )
            })
            .filter(|(multiplicity, _value)| multiplicity.is_nonzero())
            .collect()
    } else {
        panic!("Can only range check single values, not tuples.")
    }
}

/// Generate a limb lookup trace from `rangecheck_trace`
///
/// This is used by cpu trace to do direct u8 lookups
#[must_use]
pub(crate) fn generate_rangecheck_u8_trace<F: RichField>(
    rangecheck_trace: &[RangeCheckColumnsView<F>],
    memory_trace: &[Memory<F>],
) -> Vec<RangeCheckU8<F>> {
    let mut multiplicities = [0u64; 256];
    RangeCheckU8LookupTable::lookups()
        .looking_tables
        .into_iter()
        .flat_map(|looking_table| match looking_table.kind {
            TableKind::RangeCheck => extract_with_mul(rangecheck_trace, &looking_table),
            TableKind::Memory => extract_with_mul(memory_trace, &looking_table),
            other => unimplemented!("Can't range check {other:?} tables"),
        })
        .for_each(|(multiplicity, limb)| {
            let limb: u8 = F::to_canonical_u64(&limb).try_into().unwrap();
            multiplicities[limb as usize] += multiplicity.to_canonical_u64();
        });
    (0..=u8::MAX)
        .map(|limb| RangeCheckU8 {
            value: F::from_canonical_u8(limb),
            multiplicity: F::from_canonical_u64(multiplicities[limb as usize]),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::util::execute_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, PrimeField64};

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::generate_poseidon2_output_bytes_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
        generate_io_transcript_trace,
    };
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::generation::rangecheck::generate_rangecheck_trace;
    use crate::generation::register::generate_register_trace;
    use crate::generation::registerinit::generate_register_init_trace;

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

        let (_skeleton_rows, cpu_rows) = generate_cpu_trace::<F>(&record);
        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let io_memory_private = generate_io_memory_private_trace(&record.executed);
        let io_memory_public = generate_io_memory_public_trace(&record.executed);
        let io_transcript = generate_io_transcript_trace(&record.executed);
        let poseidon2_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_trace);
        let memory_rows = generate_memory_trace::<F>(
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private,
            &io_memory_public,
            &poseidon2_trace,
            &poseidon2_output_bytes,
        );
        let register_init = generate_register_init_trace(&record);
        let (_, _, register_rows) = generate_register_trace(
            &cpu_rows,
            &io_memory_private,
            &io_memory_public,
            &io_transcript,
            &register_init,
        );
        let rangecheck_rows =
            generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows, &register_rows);

        let trace = generate_rangecheck_u8_trace(&rangecheck_rows, &memory_rows);

        for row in &trace {
            // TODO(bing): more comprehensive test once we rip out the old trace gen logic.
            // For now, just assert that all values are capped by u8::MAX.
            assert!(u8::try_from(u16::try_from(row.value.to_canonical_u64()).unwrap()).is_ok());
        }

        assert_eq!(trace[0].value, F::from_canonical_u8(0));
        assert_eq!(trace[0].multiplicity, F::from_canonical_u64(24));
        assert_eq!(trace[255].value, F::from_canonical_u8(u8::MAX));
        assert_eq!(trace[255].multiplicity, F::from_canonical_u64(17));
    }
}
