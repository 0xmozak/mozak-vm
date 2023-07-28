use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::{MAP as cpu_map, NUM_CPU_COLS};
use crate::lookup::permute_cols;
use crate::rangecheck::columns;
use crate::rangecheck::columns::MAP;

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
fn limbs_from_u32<F: RichField>(value: u32) -> (F, F) {
    (
        F::from_canonical_u32(value >> 16),
        F::from_canonical_u32(value & 0xffff),
    )
}

fn push_rangecheck_row<F: RichField>(
    trace: &mut [Vec<F>],
    rangecheck_row: [F; columns::NUM_RC_COLS],
) {
    for (i, col) in rangecheck_row.iter().enumerate() {
        trace[i].push(*col);
    }
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
    cpu_trace: &[Vec<F>; NUM_CPU_COLS],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![]; columns::NUM_RC_COLS];

    for (i, _) in cpu_trace[0].iter().enumerate() {
        let mut rangecheck_row = [F::ZERO; columns::NUM_RC_COLS];
        if cpu_trace[cpu_map.ops.add][i].is_one() {
            let dst_val = u32::try_from(cpu_trace[cpu_map.dst_value][i].to_canonical_u64())
                .expect("casting COL_DST_VALUE to u32 should succeed");
            let (limb_hi, limb_lo) = limbs_from_u32(dst_val);
            rangecheck_row[MAP.val] = cpu_trace[cpu_map.dst_value][i];
            rangecheck_row[MAP.limb_hi] = limb_hi;
            rangecheck_row[MAP.limb_lo] = limb_lo;
            rangecheck_row[MAP.cpu_filter] = F::ONE;

            push_rangecheck_row(&mut trace, rangecheck_row);
        }
    }

    // Pad our trace to max(RANGE_CHECK_U16_SIZE, trace[0].len())
    trace = pad_rc_trace(trace);

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    trace[MAP.fixed_range_check_u16] = (0..RANGE_CHECK_U16_SIZE as u64)
        .map(F::from_noncanonical_u64)
        .collect();
    let num_rows = trace[MAP.val].len();
    trace[MAP.fixed_range_check_u16].resize(num_rows, F::from_canonical_u16(u16::MAX));

    // This permutation is done in accordance to the [Halo2 lookup argument
    // spec](https://zcash.github.io/halo2/design/proving-system/lookup.html)
    let (col_input_permuted, col_table_permuted) =
        permute_cols(&trace[MAP.limb_lo], &trace[MAP.fixed_range_check_u16]);

    // We need a column for the lower limb.
    trace[MAP.limb_lo_permuted] = col_input_permuted;
    trace[MAP.fixed_range_check_u16_permuted_lo] = col_table_permuted;

    let (col_input_permuted, col_table_permuted) =
        permute_cols(&trace[MAP.limb_hi], &trace[MAP.fixed_range_check_u16]);

    // And we also need a column for the upper limb.
    trace[MAP.limb_hi_permuted] = col_input_permuted;
    trace[MAP.fixed_range_check_u16_permuted_hi] = col_table_permuted;

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
    use mozak_vm::test_utils::simple_test;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;

    #[test]
    fn test_add_instruction_inserts_rangecheck() {
        type F = GoldilocksField;
        let record = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            // Use values that would become limbs later
            &[(6, 0xffff), (7, 0xffff)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows);

        // Check values that we are interested in
        assert_eq!(trace[MAP.cpu_filter][0], F::ONE);
        assert_eq!(trace[MAP.cpu_filter][1], F::ONE);
        assert_eq!(trace[MAP.val][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[MAP.val][1], GoldilocksField(93));
        assert_eq!(trace[MAP.limb_hi][0], GoldilocksField(0x0001));
        assert_eq!(trace[MAP.limb_lo][0], GoldilocksField(0xfffe));
        assert_eq!(trace[MAP.limb_lo][1], GoldilocksField(93));

        // Ensure rest of trace is zeroed out
        for cpu_filter in &trace[MAP.cpu_filter][2..] {
            assert_eq!(cpu_filter, &F::ZERO);
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
}
