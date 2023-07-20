use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::lookup::permute_cols;
use crate::rangecheck::columns;

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
    let extra = len - trace[0].len();

    for col in &mut trace {
        col.extend(vec![F::ZERO; extra]);
    }

    trace
}

/// Converts a u32 into 2 u16 limbs represented in [`RichField`].
fn limbs_from_u32<F: RichField>(value: u32) -> (F, F) {
    let limb_hi = u16::try_from(value >> 16).unwrap();
    let limb_lo = u16::try_from(value & 0xffff).unwrap();

    (
        F::from_noncanonical_u64(limb_hi.into()),
        F::from_noncanonical_u64(limb_lo.into()),
    )
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
    cpu_trace: &[Vec<F>; cpu_cols::NUM_CPU_COLS],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![]; columns::NUM_RC_COLS];

    for (i, _) in cpu_trace[0].iter().enumerate() {
        if cpu_trace[cpu_cols::S_ADD][i].is_one() {
            let dst_val = u32::try_from(cpu_trace[cpu_cols::DST_VALUE][i].to_canonical_u64())
                .expect("casting COL_DST_VALUE to u32 should succeed");
            let (limb_hi, limb_lo) = limbs_from_u32(dst_val);
            trace[columns::DST_VALUE].push(cpu_trace[cpu_cols::DST_VALUE][i]);
            trace[columns::LIMB_HI].push(limb_hi);
            trace[columns::LIMB_LO].push(limb_lo);
            trace[columns::CPU_FILTER].push(F::ONE);
        }
    }

    // Pad our trace to max(RANGE_CHECK_U16_SIZE, trace[0].len())
    trace = pad_rc_trace(trace);

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    trace[columns::FIXED_RANGE_CHECK_U16] = (0..RANGE_CHECK_U16_SIZE as u64)
        .map(F::from_noncanonical_u64)
        .collect();

    // This permutation is done in accordance to the [Halo2 lookup argument
    // spec](https://zcash.github.io/halo2/design/proving-system/lookup.html)
    let (col_input_permuted, col_table_permuted) = permute_cols(
        &trace[columns::LIMB_LO],
        &trace[columns::FIXED_RANGE_CHECK_U16],
    );

    // We need a column for the lower limb.
    trace[columns::LIMB_LO_PERMUTED] = col_input_permuted;
    trace[columns::FIXED_RANGE_CHECK_U16_PERMUTED_LO] = col_table_permuted;

    let (col_input_permuted, col_table_permuted) = permute_cols(
        &trace[columns::LIMB_HI],
        &trace[columns::FIXED_RANGE_CHECK_U16],
    );

    // And we also need a column for the upper limb.
    trace[columns::LIMB_HI_PERMUTED] = col_input_permuted;
    trace[columns::FIXED_RANGE_CHECK_U16_PERMUTED_HI] = col_table_permuted;
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
        assert_eq!(trace[columns::CPU_FILTER][0], F::ONE);
        assert_eq!(trace[columns::CPU_FILTER][1], F::ONE);
        assert_eq!(trace[columns::DST_VALUE][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[columns::DST_VALUE][1], GoldilocksField(93));
        assert_eq!(trace[columns::LIMB_HI][0], GoldilocksField(0x0001));
        assert_eq!(trace[columns::LIMB_LO][0], GoldilocksField(0xfffe));
        assert_eq!(trace[columns::LIMB_LO][1], GoldilocksField(93));

        // Ensure rest of trace is zeroed out
        for cpu_filter in &trace[columns::CPU_FILTER][2..] {
            assert_eq!(cpu_filter, &F::ZERO);
        }
        for value in &trace[columns::DST_VALUE][2..] {
            assert_eq!(value, &F::ZERO);
        }
        for limb_hi in &trace[columns::LIMB_HI][1..] {
            assert_eq!(limb_hi, &F::ZERO);
        }
        for limb_lo in &trace[columns::LIMB_LO][2..] {
            assert_eq!(limb_lo, &F::ZERO);
        }
    }
}
