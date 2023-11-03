use std::collections::HashMap;
use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::memory::columns::Memory;
use crate::rangecheck::columns::{MultiplicityView, RangeCheckColumnsView, MAP};
use crate::stark::lookup::{rangechecks_u32, Looking};
use crate::stark::mozak_stark::{Lookups, RangecheckTable, Table, TableKind};
use crate::stark::utils::transpose_trace;

#[must_use]
pub fn limbs_from_u32(value: u32) -> [u8; 4] { value.to_le_bytes() }

pub fn extract<'a, F: RichField, V>(trace: &[V], looking_table: &Table<F>) -> Vec<F>
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
    memory_trace: &[Memory<F>],
) -> Vec<RangeCheckColumnsView<F>> {
    let mut multiplicities: HashMap<u32, u32> = HashMap::new();
    let mut num_mults: usize = 0;

    rangechecks_u32()
        .looking_tables
        .into_iter()
        .for_each(|looking_table| {
            match looking_table.kind {
                TableKind::Cpu => cpu_trace.iter().map(|s| s.dst_value).collect::<Vec<_>>(),
                // TableKind::Memory => extract(memory_trace, &looking_table),
                other => unimplemented!("Can't range check {other:#?} tables"),
            }
            .into_iter()
            .for_each(|v| {
                let val = u32::try_from(v.to_canonical_u64())
                    .expect("casting value to u32 should succeed");

                if let Some(x) = multiplicities.get_mut(&val) {
                    *x += 1;
                } else {
                    multiplicities.insert(val, 1);
                }
                num_mults += 1;
            });
        });

    let mut trace: Vec<RangeCheckColumnsView<F>> = Vec::with_capacity(num_mults);

    for (i, (value, multiplicity)) in multiplicities.iter().enumerate() {
        trace.push(RangeCheckColumnsView {
            limbs: limbs_from_u32(*value).map(F::from_canonical_u8),
            filter: F::ONE,
            logup_u32: MultiplicityView {
                value: F::from_canonical_u32(*value),
                multiplicity: F::from_canonical_u32(*multiplicity),
            },
        });
    }
    trace.resize(
        trace.len().next_power_of_two(),
        RangeCheckColumnsView::default(),
    );

    println!("rc: {} {}", multiplicities.len(), trace.len());
    trace
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;

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
        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&program, &record.executed);
        let fullword_memory = generate_fullword_memory_trace(&program, &record.executed);
        let memory_rows = generate_memory_trace::<F>(
            &program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
        );
        let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);

        // Check values that we are interested in
        // rangecheck_rows.iter().all(|f| f.is_one() || f.is_zero());
        //        assert_eq!(trace[0].limbs[0], GoldilocksField(0xfe));
        //        assert_eq!(trace[0].limbs[1], GoldilocksField(0xff));
        //        assert_eq!(trace[0].limbs[2], GoldilocksField(0x01));
        //        assert_eq!(trace[0].limbs[3], GoldilocksField(0x00));
        //        assert_eq!(trace[1].limbs[0], GoldilocksField(0));
    }
}
