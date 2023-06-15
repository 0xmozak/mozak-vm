use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::lookup::permuted_cols;
use crate::rangecheck::columns;
use crate::utils::{augment_dst, from_};

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16;

/// Pad the trace to a power of 2.
///
/// # Panics
/// There's an assert that makes sure all columns passed in have the same
/// length.
#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<Vec<F>>) -> Vec<Vec<F>> {
    let len = trace[0].len();
    if let Some(padded_len) = len.checked_next_power_of_two() {
        trace[columns::VAL..columns::NUM_RC_COLS]
            .iter_mut()
            .for_each(|col| {
                col.extend(vec![*col.last().unwrap(); padded_len - len]);
            });
    }
    trace
}

/// Generate a trace table for range checks, used in generating a
/// `RangeCheckStark`.
///
/// # Panics
///
/// Panics if:
/// 1. conversion of `dst_val` from u32 to u16 fails when splitting
///    into limbs,
/// 2. trace width does not match the number of columns.
pub fn generate_rangecheck_trace<F: RichField>(
    step_rows: &[Row],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; RANGE_CHECK_U16_SIZE]; columns::NUM_RC_COLS];

    for (i, (s, dst_val)) in augment_dst(step_rows.iter().map(|row| &row.state)).enumerate() {
        let inst = s.current_instruction();

        #[allow(clippy::single_match)]
        match inst.op {
            Op::ADD => {
                let limb_hi = u16::try_from(dst_val >> 8).unwrap();
                let limb_lo = u16::try_from(dst_val & 0xffff).unwrap();
                trace[columns::VAL][i] = from_(dst_val);
                trace[columns::LIMB_HI][i] = from_(limb_hi);
                trace[columns::LIMB_LO][i] = from_(limb_lo);
                trace[columns::CPU_FILTER][i] = F::ONE;
            }
            _ => {}
        }
    }

    trace[columns::FIXED_RANGE_CHECK_U16] =
        (0..RANGE_CHECK_U16_SIZE).map(|i| from_(i as u64)).collect();

    let (permuted_inputs, permuted_table) = permuted_cols(
        &trace[columns::LIMB_LO],
        &trace[columns::FIXED_RANGE_CHECK_U16],
    );

    trace[columns::LIMB_LO_PERMUTED] = permuted_inputs;
    trace[columns::FIXED_RANGE_CHECK_U16_PERMUTED_LO] = permuted_table;

    let (permuted_inputs, permuted_table) = permuted_cols(
        &trace[columns::LIMB_HI],
        &trace[columns::FIXED_RANGE_CHECK_U16],
    );

    trace[columns::LIMB_HI_PERMUTED] = permuted_inputs;
    trace[columns::FIXED_RANGE_CHECK_U16_PERMUTED_HI] = permuted_table;
    let trace = pad_trace(trace);

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
    use plonky2::field::{goldilocks_field::GoldilocksField, types::Field};

    use super::*;
    #[test]
    fn test_add_instruction_inserts_rangecheck() {
        type F = GoldilocksField;
        let (rows, _) = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            &[(6, 100), (7, 100)],
        );

        let trace = generate_rangecheck_trace::<F>(&rows);
        for (idx, column) in trace.iter().enumerate() {
            if idx == columns::CPU_FILTER {
                for (i, column) in column.iter().enumerate() {
                    // Only the first two instructions are ADD, which require a range check
                    if i < 2 {
                        assert_eq!(column, &F::ONE);
                    } else {
                        assert_eq!(column, &F::ZERO);
                    }
                }
            }

            if idx == columns::VAL {
                for (i, column) in column.iter().enumerate() {
                    match i {
                        // 100 + 100 = 200
                        0 => assert_eq!(column, &GoldilocksField(200)),
                        // exit instruction
                        1 => assert_eq!(column, &GoldilocksField(93)),
                        _ => assert_eq!(column, &F::ZERO),
                    }
                }
            }
        }
    }
}
