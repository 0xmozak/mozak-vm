use mozak_vm::instruction::Op;
use mozak_vm::state::Aux;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::lookup::permute_cols;
use crate::rangecheck::columns;

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16;

/// Initializes the rangecheck trace table to the size of 2^k rows in
/// preparation for the Halo2 lookup argument.
///
/// Note that by right the column to be checked (A) and the fixed column (S)
/// have to be extended by dummy values known to be in the fixed column if they
/// are not of size 2^k, but because our fixed column is a range from 0..2^16-1,
/// initializing our trace to all [`F::ZERO`]s takes care of this step by
/// default.
#[must_use]
fn init_padded_rc_trace<F: RichField>(len: usize) -> Vec<Vec<F>> {
    vec![vec![F::ZERO; len.next_power_of_two()]; columns::NUM_RC_COLS]
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
    step_rows: &[Row],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace = init_padded_rc_trace(step_rows.len().max(RANGE_CHECK_U16_SIZE));

    for (
        i,
        Row {
            state: s,
            aux: Aux { dst_val, .. },
        },
    ) in step_rows.iter().enumerate()
    {
        let inst = s.current_instruction();

        #[allow(clippy::single_match)]
        match inst.op {
            Op::ADD => {
                let limb_hi = u16::try_from(dst_val >> 16).unwrap();
                let limb_lo = u16::try_from(dst_val & 0xffff).unwrap();
                trace[columns::VAL][i] = F::from_noncanonical_u64((*dst_val).into());
                trace[columns::LIMB_HI][i] = F::from_noncanonical_u64(limb_hi.into());
                trace[columns::LIMB_LO][i] = F::from_noncanonical_u64(limb_lo.into());
                trace[columns::CPU_FILTER][i] = F::ONE;
            }
            _ => {}
        }
    }

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    trace[columns::FIXED_RANGE_CHECK_U16] = (0..RANGE_CHECK_U16_SIZE as u64)
        .map(F::from_noncanonical_u64)
        .collect();
    let num_rows = trace[columns::VAL].len();
    trace[columns::FIXED_RANGE_CHECK_U16]
        .resize(num_rows, F::from_canonical_u64(u64::from(u16::MAX)));

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

    #[test]
    fn test_add_instruction_inserts_rangecheck() {
        type F = GoldilocksField;
        let record = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            // Use values that would become limbs later
            &[(6, 0xffff), (7, 0xffff)],
        );

        let trace = generate_rangecheck_trace::<F>(&record.executed);

        // Check values that we are interested in
        assert_eq!(trace[columns::CPU_FILTER][0], F::ONE);
        assert_eq!(trace[columns::CPU_FILTER][1], F::ONE);
        assert_eq!(trace[columns::VAL][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[columns::VAL][1], GoldilocksField(93));
        assert_eq!(trace[columns::LIMB_HI][0], GoldilocksField(0x0001));
        assert_eq!(trace[columns::LIMB_LO][0], GoldilocksField(0xfffe));
        assert_eq!(trace[columns::LIMB_LO][1], GoldilocksField(93));

        // Ensure rest of trace is zeroed out
        for cpu_filter in &trace[columns::CPU_FILTER][2..] {
            assert_eq!(cpu_filter, &F::ZERO);
        }
        for value in &trace[columns::VAL][2..] {
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
