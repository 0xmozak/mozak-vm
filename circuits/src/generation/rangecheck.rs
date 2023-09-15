use std::ops::Index;

use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::memory::columns::Memory;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::stark::mozak_stark::{Lookups, RangecheckTable, Table, TableKind};
use crate::utils::pad_trace_with_default;

/// Converts a u32 into 2 u16 limbs represented in [`RichField`].
#[must_use]
pub fn limbs_from_u32<F: RichField>(value: u32) -> (F, F) {
    (
        F::from_noncanonical_u64((value >> 16).into()),
        F::from_noncanonical_u64((value & 0xffff).into()),
    )
}

pub fn extract<'a, F: RichField, V>(trace: &[V], looking_table: &Table<F>) -> Vec<F>
where
    V: Index<usize, Output = F> + 'a, {
    if let [column] = &looking_table.columns[..] {
        trace
            .iter()
            .filter(|&row| looking_table.filter_column.eval(row).is_one())
            .map(|row| column.eval(row))
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
/// 1. conversion of u32 values to u16 limbs,
/// 2. trace width does not match the number of columns,
/// 3. attempting to range check tuples instead of single values.
#[must_use]
pub(crate) fn generate_rangecheck_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    memory_trace: &[Memory<F>],
) -> Vec<RangeCheckColumnsView<F>> {
    pad_trace_with_default(
        RangecheckTable::lookups()
            .looking_tables
            .into_iter()
            .flat_map(|looking_table| {
                match looking_table.kind {
                    TableKind::Cpu => extract(cpu_trace, &looking_table),
                    TableKind::Memory => extract(memory_trace, &looking_table),
                    other => unimplemented!("Can't range check {other:#?} tables"),
                }
                .into_iter()
                .map(move |val| {
                    let (limb_hi, limb_lo) = limbs_from_u32(
                        u32::try_from(val.to_canonical_u64())
                            .expect("casting value to u32 should succeed"),
                    );
                    RangeCheckColumnsView {
                        limb_lo,
                        limb_hi,
                        filter: F::ONE,
                    }
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::memory::generate_memory_trace;

    #[test]
    fn test_add_instruction_inserts_rangecheck() {
        type F = GoldilocksField;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            // Use values that would become limbs later
            &[],
            &[(6, 0xffff), (7, 0xffff)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let memory_rows = generate_memory_trace::<F>(&program, &record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);

        // Check values that we are interested in
        assert_eq!(trace[0].filter, F::ONE);
        assert_eq!(trace[1].filter, F::ONE);
        assert_eq!(trace[0].limb_hi, GoldilocksField(0x0001));
        assert_eq!(trace[0].limb_lo, GoldilocksField(0xfffe));
        assert_eq!(trace[1].limb_lo, GoldilocksField(0));
    }
}
