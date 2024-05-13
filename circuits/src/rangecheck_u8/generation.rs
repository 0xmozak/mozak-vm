use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::memory::columns::Memory;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck::generation::extract_with_mul;
use crate::rangecheck_u8::columns::RangeCheckU8;
use crate::stark::mozak_stark::{Lookups, RangeCheckU8LookupTable, TableKind};

/// Generate a limb lookup trace from `rangecheck_trace`
///
/// This is used by cpu trace to do direct u8 lookups
#[must_use]
pub(crate) fn generate_rangecheck_u8_trace<F: RichField>(
    rangecheck_trace: &[RangeCheckColumnsView<F>],
    memory_trace: &[Memory<F>],
) -> Vec<RangeCheckU8<F>> {
    RangeCheckU8LookupTable::lookups()
        .looking_tables
        .into_iter()
        .flat_map(|looking_table| match looking_table.kind {
            TableKind::RangeCheck => extract_with_mul(rangecheck_trace, &looking_table),
            TableKind::Memory => extract_with_mul(memory_trace, &looking_table),
            // We are trying to build this table, so we have to ignore it here.
            TableKind::RangeCheckU8 => vec![],
            other => unimplemented!("Can't range check {other:?} tables"),
        })
        .chain((0..=u8::MAX).map(|v| (F::from_canonical_u8(v), F::ZERO)))
        .into_group_map()
        .into_iter()
        .sorted_by_key(|(value, _)| value.to_noncanonical_u64())
        .map(|(value, mults)| RangeCheckU8 {
            value,
            multiplicity: mults.into_iter().sum(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, PrimeField64};

    use super::*;
    use crate::cpu::generation::generate_cpu_trace;
    use crate::memory::generation::generate_memory_trace;
    use crate::memory_fullword::generation::generate_fullword_memory_trace;
    use crate::memory_halfword::generation::generate_halfword_memory_trace;
    use crate::memory_zeroinit::generation::generate_memory_zero_init_trace;
    use crate::memoryinit::generation::generate_memory_init_trace;
    use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::rangecheck::generation::generate_rangecheck_trace;
    use crate::register::generation::{generate_register_init_trace, generate_register_trace};
    use crate::storage_device::generation::{
        generate_call_tape_trace, generate_cast_list_commitment_tape_trace,
        generate_event_tape_trace, generate_events_commitment_tape_trace,
        generate_private_tape_trace, generate_public_tape_trace, generate_self_prog_id_tape_trace,
    };

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
        let private_tape = generate_private_tape_trace(&record.executed);
        let public_tape = generate_public_tape_trace(&record.executed);
        let call_tape_rows = generate_call_tape_trace(&record.executed);
        let event_tape_rows = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&record.executed);
        let self_prog_id_tape_rows = generate_self_prog_id_tape_trace(&record.executed);
        let poseidon2_sponge_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_sponge_trace);
        let memory_rows = generate_memory_trace::<F>(
            &record.executed,
            &memory_init,
            &memory_zeroinit_rows,
            &halfword_memory,
            &fullword_memory,
            &private_tape,
            &public_tape,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &self_prog_id_tape_rows,
            &poseidon2_sponge_trace,
            &poseidon2_output_bytes,
        );
        let register_init = generate_register_init_trace(&record);
        let (_, _, register_rows) = generate_register_trace(
            &cpu_rows,
            &poseidon2_sponge_trace,
            &private_tape,
            &public_tape,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &self_prog_id_tape_rows,
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
        assert_eq!(trace[0].multiplicity, F::from_canonical_u64(48));
        assert_eq!(trace[255].value, F::from_canonical_u8(u8::MAX));
        assert_eq!(trace[255].multiplicity, F::from_canonical_u64(4));
    }
}
