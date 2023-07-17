use mozak_vm::instruction::Op;
use mozak_vm::state::Aux;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::lookup::permute_cols;
use crate::rangecheck::columns::{self, LimbKind};
use crate::utils::from_;

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

    println!("generate rangecheck trace ");
    for (
        i,
        Row {
            state: s,
            aux:
                Aux {
                    dst_val,
                    rs_1,
                    rs_2,
                    ..
                },
        },
    ) in step_rows.iter().enumerate()
    {
        let inst = s.current_instruction();

        #[allow(clippy::single_match)]
        match inst.op {
            Op::ADD => {
                let limb_hi = u16::try_from(dst_val >> 16).unwrap();
                let limb_lo = u16::try_from(dst_val & 0xffff).unwrap();
                trace[columns::VAL][i] = from_(*dst_val);
                trace[LimbKind::col(columns::VAL, LimbKind::Hi)][i] = from_(limb_hi);
                trace[LimbKind::col(columns::VAL, LimbKind::Lo)][i] = from_(limb_lo);
                trace[columns::CPU_ADD][i] = F::ONE;
            }
            Op::SLT => {
                let is_signed = inst.op == Op::SLT;

                let sign_adjust = if is_signed { 1 << 31 } else { 0 };
                let op1_fixed = rs_1.wrapping_add(sign_adjust);
                let op2_fixed = rs_2.wrapping_add(sign_adjust);
                let abs_diff_fixed: u32 = op1_fixed.abs_diff(op2_fixed);

                let limb_hi = u16::try_from(op1_fixed >> 16).unwrap();
                let limb_lo = u16::try_from(op2_fixed & 0xffff).unwrap();

                println!("generating SLT: val={} op1_fixed={}", dst_val, op1_fixed);
                trace[columns::OP1_FIXED][i] = from_(op1_fixed);

                trace[LimbKind::col(columns::OP1_FIXED, LimbKind::Hi)][i] = from_(limb_hi);
                trace[LimbKind::col(columns::OP1_FIXED, LimbKind::Lo)][i] = from_(limb_lo);
                trace[columns::CPU_ADD][i] = F::ONE;
            }

            _ => {}
        }
    }

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    trace[columns::FIXED_RANGE_CHECK_U16] =
        (0..RANGE_CHECK_U16_SIZE).map(|i| from_(i as u64)).collect();

    for idx in [columns::VAL, columns::OP1_FIXED] {
        // This permutation is done i accordance to the [Halo2 lookup argument
        // spec](https://zcash.github.io/halo2/design/proving-system/lookup.html)
        let (col_input_permuted, col_table_permuted) = permute_cols(
            &trace[LimbKind::col(idx, LimbKind::Lo)],
            &trace[columns::FIXED_RANGE_CHECK_U16],
        );

        // We need a column for the lower limb.
        trace[LimbKind::col(idx, LimbKind::LoPermuted)] = col_input_permuted;
        trace[LimbKind::col(idx, LimbKind::LoFixedPermuted)] = col_table_permuted;

        let (col_input_permuted, col_table_permuted) = permute_cols(
            &trace[LimbKind::col(idx, LimbKind::Hi)],
            &trace[columns::FIXED_RANGE_CHECK_U16],
        );

        // And we also need a column for the upper limb.
        trace[LimbKind::col(idx, LimbKind::HiPermuted)] = col_input_permuted;
        trace[LimbKind::col(idx, LimbKind::HiFixedPermuted)] = col_table_permuted;
    }

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
        assert_eq!(trace[columns::CPU_ADD][0], F::ONE);
        assert_eq!(trace[columns::CPU_ADD][1], F::ONE);
        assert_eq!(trace[columns::VAL][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[columns::VAL][1], GoldilocksField(93));
        assert_eq!(
            trace[LimbKind::col(columns::VAL, LimbKind::Hi)][0],
            GoldilocksField(0x0001)
        );
        assert_eq!(
            trace[LimbKind::col(columns::VAL, LimbKind::Lo)][0],
            GoldilocksField(0xfffe)
        );
        assert_eq!(
            trace[LimbKind::col(columns::VAL, LimbKind::Lo)][1],
            GoldilocksField(93)
        );

        // Ensure rest of trace is zeroed out
        for cpu_filter in trace[columns::CPU_ADD][2..].iter() {
            assert_eq!(cpu_filter, &F::ZERO);
        }
        for value in trace[columns::VAL][2..].iter() {
            assert_eq!(value, &F::ZERO);
        }
        for limb_hi in trace[LimbKind::col(columns::VAL, LimbKind::Hi)][1..].iter() {
            assert_eq!(limb_hi, &F::ZERO);
        }
        for limb_lo in trace[LimbKind::col(columns::VAL, LimbKind::Lo)][2..].iter() {
            assert_eq!(limb_lo, &F::ZERO);
        }
    }
}
