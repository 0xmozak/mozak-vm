use itertools::chain;
use plonky2::hash::hash_types::RichField;

use crate::columns_view::NumberOfColumns;
use crate::cpu::columns::CpuState;
use crate::lookup::permute_cols;
use crate::rangecheck::columns::{
    FixedColumnsView, RangeCheckColumnsExtended, RangeCheckColumnsView, MAP,
};
use crate::stark::utils::transpose_trace;

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
fn pad_rc_trace<F: RichField>(
    mut trace: Vec<RangeCheckColumnsView<F>>,
) -> Vec<RangeCheckColumnsView<F>> {
    let len = trace[MAP.rangecheck.val]
        .into_iter()
        .len()
        .max(RANGE_CHECK_U16_SIZE)
        .next_power_of_two();

    trace.resize(len, RangeCheckColumnsView {
        cpu_filter: F::ZERO,
        // .. and all other columns just have their last value duplicated.
        ..*trace.last().unwrap()
    });

    trace
}

/// Converts a u32 into 2 u16 limbs represented in [`RichField`].
fn limbs_from_u32<F: RichField>(value: u32) -> (F, F) {
    (
        F::from_canonical_u32(value >> 16),
        F::from_canonical_u32(value & 0xffff),
    )
}
pub fn generate_rangecheck_trace_extended<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> RangeCheckColumnsExtended<Vec<F>> {
    let mut rangecheck_trace = transpose_trace(generate_rangecheck_trace(cpu_trace));
    let fixed = generate_fixed_trace(&mut rangecheck_trace);

    chain!(rangecheck_trace, fixed).collect()
}

#[must_use]
pub fn generate_fixed_trace<F: RichField>(trace: &mut Vec<Vec<F>>) -> Vec<Vec<F>> {
    let mut fixed_trace: Vec<Vec<F>> = vec![vec![]; FixedColumnsView::<()>::NUMBER_OF_COLUMNS];

    let len = trace[MAP.rangecheck.val]
        .len()
        .max(RANGE_CHECK_U16_SIZE)
        .next_power_of_two();

    fixed_trace
        .iter_mut()
        .for_each(move |c| c.resize(len, F::ZERO));

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    fixed_trace[MAP.permuted.fixed_range_check_u16 - trace.len()] = (0..RANGE_CHECK_U16_SIZE
        as u64)
        .map(F::from_noncanonical_u64)
        .collect();

    fixed_trace[MAP.permuted.fixed_range_check_u16 - trace.len()]
        .resize(len, F::from_canonical_u64(u64::from(u16::MAX)));

    trace[MAP.rangecheck.limb_lo].resize(len, F::ZERO);
    // This permutation is done in accordance to the [Halo2 lookup argument
    // spec](https://zcash.github.io/halo2/design/proving-system/lookup.html)
    let (col_input_permuted, col_table_permuted) = permute_cols(
        &trace[MAP.rangecheck.limb_lo],
        &fixed_trace[MAP.permuted.fixed_range_check_u16 - trace.len()],
    );

    // We need a column for the lower limb.
    fixed_trace[MAP.permuted.limb_lo_permuted - trace.len()] = col_input_permuted;
    fixed_trace[MAP.permuted.fixed_range_check_u16_permuted_lo - trace.len()] = col_table_permuted;

    let (col_input_permuted, col_table_permuted) = permute_cols(
        &trace[MAP.rangecheck.limb_hi],
        &fixed_trace[MAP.permuted.fixed_range_check_u16 - trace.len()],
    );

    // And we also need a column for the upper limb.
    fixed_trace[MAP.permuted.limb_hi_permuted - trace.len()] = col_input_permuted;
    fixed_trace[MAP.permuted.fixed_range_check_u16_permuted_hi - trace.len()] = col_table_permuted;

    fixed_trace
}

/// Generates a trace table for range checks, used in building a
/// `RangeCheckStark` proof.
///
/// # Panics
///
/// Panics if:
/// 1. conversion of `dst_val` from u32 to u16 fails when splitting into limbs,
/// 2. trace width does not match the number of columns.
#[must_use]
pub fn generate_rangecheck_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> Vec<RangeCheckColumnsView<F>> {
    let mut trace: Vec<RangeCheckColumnsView<F>> = vec![];

    for cpu_row in cpu_trace {
        if cpu_row.inst.ops.add.is_one() {
            let val = u32::try_from(cpu_row.dst_value.to_canonical_u64())
                .expect("casting COL_DST_VALUE to u32 should succeed");
            let (limb_hi, limb_lo) = limbs_from_u32(val);

            let row = RangeCheckColumnsView {
                val: cpu_row.dst_value,
                limb_hi,
                limb_lo,
                cpu_filter: F::ONE,
            };

            trace.push(row);
        }
    }

    trace = pad_rc_trace(trace);

    trace
}

#[cfg(test)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::utils::from_u32;

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

        let cpu_rows = generate_cpu_trace::<F>(&program, &record.executed);
        let trace = generate_rangecheck_trace_extended::<F>(&cpu_rows);

        // Check values that we are interested in
        assert_eq!(trace[MAP.rangecheck.cpu_filter][0], F::ONE);
        assert_eq!(trace[MAP.rangecheck.cpu_filter][1], F::ONE);
        assert_eq!(trace[MAP.rangecheck.val][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[MAP.rangecheck.val][1], GoldilocksField(93));
        assert_eq!(trace[MAP.rangecheck.limb_hi][0], GoldilocksField(0x0001));
        assert_eq!(trace[MAP.rangecheck.limb_lo][0], GoldilocksField(0xfffe));
        assert_eq!(trace[MAP.rangecheck.limb_lo][1], GoldilocksField(93));

        // Ensure rest of trace is zeroed out
        for cpu_filter in &trace[MAP.rangecheck.cpu_filter][2..] {
            assert_eq!(cpu_filter, &F::ZERO);
        }
        for value in &trace[MAP.rangecheck.val][2..] {
            assert_eq!(value, &from_u32::<F>(93));
        }
        for limb_hi in &trace[MAP.rangecheck.limb_hi][1..] {
            assert_eq!(limb_hi, &F::ZERO);
        }
        for limb_lo in &trace[MAP.rangecheck.limb_lo][2..] {
            assert_eq!(limb_lo, &from_u32::<F>(93));
        }
    }
}
