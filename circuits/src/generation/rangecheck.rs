use std::borrow::Borrow;
use std::ops::Index;

use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::memory::columns::Memory;
use crate::rangecheck::columns::{self, RangeCheckColumnsView, MAP};
use crate::stark::mozak_stark::{Lookups, RangecheckTable, Table, TableKind};

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16;

/// Pad the rangecheck trace table to the size of 2^k rows in
/// preparation for the Halo2 lookup argument.
///
/// Note that by right the column to be checked (A) and the fixed column (S)
/// have to be extended by dummy values known to be in the fixed column if they
/// are not of size 2^k, but because our fixed column is a range from 0..2^16-1,
/// initializing our trace to all [`F::ZERO`]s takes care of this step by
/// default.
#[must_use]
fn pad_rc_trace<F: RichField>(mut trace: Vec<Vec<F>>) -> Vec<Vec<F>> {
    let len = trace[0].len().max(RANGE_CHECK_U16_SIZE).next_power_of_two();

    trace.iter_mut().for_each(move |c| c.resize(len, F::ZERO));

    trace
}

/// Converts a u32 into 2 u16 limbs represented in [`RichField`].
#[must_use]
pub fn limbs_from_u32(value: u32) -> (u16, u16) { ((value >> 16) as u16, (value & 0xffff) as u16) }

fn push_rangecheck_row<F: RichField>(
    trace: &mut [Vec<F>],
    rangecheck_row: &[F; columns::NUM_RC_COLS],
) {
    for (i, col) in rangecheck_row.iter().enumerate() {
        trace[i].push(*col);
    }
}

#[allow(clippy::missing_panics_doc)]
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
pub fn generate_rangecheck_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    memory_trace: &[Memory<F>],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![]; columns::NUM_RC_COLS];
    let mut multiplicities = [F::ZERO; RANGE_CHECK_U16_SIZE];

    for looking_table in RangecheckTable::lookups().looking_tables {
        let values = match looking_table.kind {
            TableKind::Cpu => extract(cpu_trace, &looking_table),
            TableKind::Memory => extract(memory_trace, &looking_table),
            other => unimplemented!("Can't range check {other:#?} tables"),
        };

        let mut i = 0;
        for val in values {
            let (limb_hi, limb_lo) = limbs_from_u32(
                u32::try_from(val.to_canonical_u64()).expect("casting value to u32 should succeed"),
            );
            let rangecheck_row = RangeCheckColumnsView {
                val,
                limb_lo: F::from_canonical_u16(limb_lo),
                limb_hi: F::from_canonical_u16(limb_hi),
                filter: F::ONE,
                ..Default::default()
            };

            if limb_hi == 0 || limb_lo == 0 {
                println!("zero: {i} {:?} {} {}", val, limb_hi, limb_lo);
            }
            if limb_hi == 0 && limb_lo == 0 {
                println!("both: {i} {:?} {} {}", val, limb_hi, limb_lo);
            }
            multiplicities[limb_hi as usize] += F::ONE;
            multiplicities[limb_lo as usize] += F::ONE;
            push_rangecheck_row(&mut trace, rangecheck_row.borrow());
            i += 1;
        }
    }
    // Pad our trace to max(RANGE_CHECK_U16_SIZE, trace[0].len())
    trace = pad_rc_trace(trace);
    // println!("mults: {}", multiplicities);
    trace[MAP.multiplicities] = multiplicities.to_vec();

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    trace[MAP.fixed_range_check_u16] = (0..RANGE_CHECK_U16_SIZE as u64)
        .map(F::from_noncanonical_u64)
        .collect();
    let num_rows = trace[MAP.val].len();
    trace[MAP.multiplicities].resize(num_rows, F::ZERO);
    trace[MAP.fixed_range_check_u16].resize(num_rows, F::from_canonical_u64(u64::from(u16::MAX)));

    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            columns::NUM_RC_COLS,
            v.len()
        )
    })
}

#[cfg(test)]
mod tests {
    use std::iter::repeat;

    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::memory::generate_memory_trace;
    type F = GoldilocksField;

    #[test]
    fn test_generation_single() {
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
            &[],
            &[(6, 0xffff), (7, 0xffff)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let memory_rows = generate_memory_trace::<F>(&program, &record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);

        println!("trace: {:?}", trace[0].len());

        trace.iter().for_each(|c| {
            assert_eq!(c.len(), RANGE_CHECK_U16_SIZE);
        });
        // Check values that we are interested in
        assert_eq!(trace[MAP.filter][0], F::ONE);
        assert_eq!(trace[MAP.filter][1], F::ONE);
        assert_eq!(trace[MAP.val][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[MAP.val][1], GoldilocksField(0));
        assert_eq!(trace[MAP.limb_hi][0], GoldilocksField(0x0001));
        assert_eq!(trace[MAP.limb_lo][0], GoldilocksField(0xfffe));
        assert_eq!(trace[MAP.limb_lo][1], GoldilocksField(0));

        // Ensure rest of trace is zeroed out
        for filter in &trace[MAP.filter][2..] {
            assert_eq!(filter, &F::ZERO);
        }
        for value in &trace[MAP.val][2..] {
            assert_eq!(value, &F::ZERO);
        }
        for limb_hi in &trace[MAP.limb_hi][1..] {
            assert_eq!(limb_hi, &F::ZERO);
        }
        for limb_lo in &trace[MAP.limb_lo][2..] {
            assert_eq!(limb_lo, &F::ZERO);
        }
    }

    #[test]
    fn test_generation_big() {
        fn generate_inst(imm: u32) -> Instruction {
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    imm,
                    ..Args::default()
                },
            }
        }
        let (program, record) = simple_test_code(
            &(0..RANGE_CHECK_U16_SIZE)
                .map(|i| generate_inst(i as u32))
                .collect::<Vec<Instruction>>(),
            &[],
            &[(6, 0xffff)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let memory_rows = generate_memory_trace::<F>(&program, &record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);

        for (i, m) in trace[MAP.multiplicities].iter().enumerate() {
            if m.is_zero() && i < RANGE_CHECK_U16_SIZE {
                println!("mults[{}]: {:?}", i, m);
            }
        }

        let trace_len = (RANGE_CHECK_U16_SIZE + 1).next_power_of_two();

        // Check each column's length.
        trace.iter().for_each(|c| {
            assert_eq!(c.len(), trace_len);
        });

        // Check multiplicity column.
        // Expect:
        // Multiplicity of 4 for 0:
        //   1st instruction's limb_hi == 0
        //   2nd instruction's limb_lo == 0
        //   last instruction, both limbs == 0
        trace[MAP.multiplicities]
            .iter()
            .enumerate()
            .for_each(|(i, m)| match i {
                0 => assert!(
                    m == &F::from_canonical_u8(4),
                    "expected multiplicity of 4 for value 0, got {m}"
                ),
                1 => assert!(
                    m == &F::from_canonical_u32(65536),
                    "expected multiplicity of 65536 for value 1, got {m}"
                ),
                RANGE_CHECK_U16_SIZE.. => assert!(
                    m.is_zero(),
                    "expected multiplicity of 0 for value {i}, got {m}"
                ),
                _ => assert!(
                    m.is_one(),
                    "expected multiplicity of 1 for value {i}, got {m}"
                ),
            });

        trace[MAP.limb_lo].iter().enumerate().for_each(|(i, l)| {
            assert_eq!(
                l,
                &F::from_canonical_u64((0xffff_u32).wrapping_add(i as u32).into())
            )
        });

        // Check values that we are interested in
        assert_eq!(trace[MAP.filter][0], F::ONE);
        assert_eq!(trace[MAP.filter][1], F::ONE);
        assert_eq!(trace[MAP.val][0], GoldilocksField(0x0000_ffff));
        assert_eq!(trace[MAP.val][1], GoldilocksField(0x0001_0000));

        // Ensure rest of trace is zeroed out
        for filter in &trace[MAP.filter][RANGE_CHECK_U16_SIZE..] {
            assert_eq!(filter, &F::ZERO);
        }
        for value in &trace[MAP.val][RANGE_CHECK_U16_SIZE..] {
            assert_eq!(value, &F::ZERO);
        }
        for limb_hi in &trace[MAP.limb_hi][RANGE_CHECK_U16_SIZE..] {
            assert_eq!(limb_hi, &F::ZERO);
        }
        for limb_lo in &trace[MAP.limb_lo][RANGE_CHECK_U16_SIZE..] {
            assert_eq!(limb_lo, &F::ZERO);
        }
    }
}
