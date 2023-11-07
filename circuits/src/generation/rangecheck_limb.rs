use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::multiplicity_view::MultiplicityView;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_limb::columns::RangeCheckLimb;
use crate::stark::mozak_stark::{LimbTable, Lookups, Table, TableKind};

/// extract the values with multiplicity nonzero
pub fn extract_with_mul<'a, F: RichField, V>(trace: &[V], looking_table: &Table<F>) -> Vec<(F, F)>
where
    V: Index<usize, Output = F> + 'a, {
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

#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    rangecheck_trace: &[RangeCheckColumnsView<F>],
) -> Vec<RangeCheckLimb<F>> {
    let mut multiplicities = [0u64; 256];
    LimbTable::lookups()
        .looking_tables
        .into_iter()
        .flat_map(|looking_table| match looking_table.kind {
            TableKind::RangeCheck => extract_with_mul(rangecheck_trace, &looking_table),
            TableKind::Cpu => extract_with_mul(cpu_trace, &looking_table),
            other => unimplemented!("Can't range check {other:?} tables"),
        })
        .for_each(|(multiplicity, limb)| {
            let limb: u8 = F::to_canonical_u64(&limb).try_into().unwrap();
            multiplicities[limb as usize] += multiplicity.to_canonical_u64();
        });
    (0..=u8::MAX)
        .map(|limb| RangeCheckLimb {
            multiplicity_view: MultiplicityView {
                value: F::from_canonical_u8(limb),
                multiplicity: F::from_canonical_u64(multiplicities[limb as usize]),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, PrimeField64};

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::generate_io_memory_trace;
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::generation::rangecheck::generate_rangecheck_trace;

    #[test]
    fn test_generate_trace() {
        type F = GoldilocksField;
        let (program, record) = simple_test_code(
            &[Instruction {
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

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&program, &record.executed);
        let fullword_memory = generate_fullword_memory_trace(&program, &record.executed);
        let io_memory = generate_io_memory_trace(&program, &record.executed);
        let poseidon2_trace = generate_poseidon2_sponge_trace(&record.executed);
        let memory_rows = generate_memory_trace::<F>(
            &program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory,
            &poseidon2_trace,
        );
        let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);

        let trace = generate_rangecheck_limb_trace(&cpu_rows, &rangecheck_rows);

        for row in &trace {
            // TODO(bing): more comprehensive test once we rip out the old trace gen logic.
            // For now, just assert that all values are capped by u8::MAX.
            assert!(u8::try_from(
                u16::try_from(row.multiplicity_view.value.to_canonical_u64()).unwrap()
            )
            .is_ok());
        }

        assert_eq!(trace[0].multiplicity_view.value, F::from_canonical_u8(0));
        assert_eq!(
            trace[0].multiplicity_view.multiplicity,
            F::from_canonical_u64(8)
        );
        assert_eq!(
            trace[255].multiplicity_view.value,
            F::from_canonical_u8(u8::MAX)
        );
        assert_eq!(
            trace[255].multiplicity_view.multiplicity,
            F::from_canonical_u64(8)
        );
    }
}
