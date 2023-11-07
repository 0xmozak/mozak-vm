use std::collections::HashMap;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use super::rangecheck::extract;
use crate::cpu::columns::CpuState;
use crate::multiplicity_view::MultiplicityView;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_limb::columns::RangeCheckLimb;
use crate::stark::mozak_stark::{LimbTable, Lookups, TableKind};

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<RangeCheckLimb<F>>) -> Vec<RangeCheckLimb<F>> {
    let len = trace.len().next_power_of_two().max(4);
    trace.resize(len, RangeCheckLimb {
        filter: F::ZERO,
        element: F::from_canonical_u8(u8::MAX),
        multiplicity_view: MultiplicityView::default(),
    });
    trace
}

#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    rangecheck_trace: &[RangeCheckColumnsView<F>],
) -> Vec<RangeCheckLimb<F>> {
    let mut multiplicities: HashMap<u8, u64> = HashMap::new();

    let mut trace =
        pad_trace(
            LimbTable::lookups()
                .looking_tables
                .into_iter()
                .flat_map(|looking_table| match looking_table.kind {
                    TableKind::RangeCheck => extract(rangecheck_trace, &looking_table),
                    TableKind::Cpu => extract(cpu_trace, &looking_table),
                    other => unimplemented!("Can't range check {other:?} tables"),
                })
                .map(|limb| F::to_canonical_u64(&limb))
                .sorted()
                .merge_join_by(0..=u64::from(u8::MAX), u64::cmp)
                .map(|value_or_dummy| {
                    let filter = u64::from(value_or_dummy.has_left());
                    let val = value_or_dummy.into_left();
                    multiplicities
                        .entry(u8::try_from(val).expect(
                            "values should be valid u8 values in the 64-bit GoldilocksField",
                        ))
                        .and_modify(|e| *e += 1)
                        .or_default();

                    RangeCheckLimb {
                        filter,
                        element: val,
                        multiplicity_view: MultiplicityView::default(),
                    }
                    .map(F::from_noncanonical_u64)
                })
                .collect::<Vec<_>>(),
        );

    for (i, (value, multiplicity)) in multiplicities.into_iter().enumerate() {
        trace[i].multiplicity_view.value = F::from_canonical_u8(value);
        trace[i].multiplicity_view.multiplicity = F::from_canonical_u64(multiplicity);
    }

    trace
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::PrimeField64;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
    };
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
        let io_memory_private = generate_io_memory_private_trace(&program, &record.executed);
        let io_memory_public = generate_io_memory_public_trace(&program, &record.executed);
        let poseidon2_trace = generate_poseidon2_sponge_trace(&record.executed);
        let memory_rows = generate_memory_trace::<F>(
            &program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private,
            &io_memory_public,
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
    }
}
