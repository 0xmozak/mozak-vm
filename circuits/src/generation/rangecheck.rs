use itertools::Itertools;
use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::lookup::permuted_cols;
use crate::rangecheck::columns;
use crate::utils::from_;

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16;

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

pub fn generate_rangecheck_trace<F: RichField>(rows: &[Row]) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; RANGE_CHECK_U16_SIZE]; columns::NUM_RC_COLS];

    for (i, (s, ns)) in rows.iter().tuple_windows().enumerate() {
        let inst = s.state.current_instruction();

        match inst.op {
            Op::ADD => {
                let val = ns.state.get_register_value(usize::from(inst.data.rd));
                let limb_hi = (val >> 8) as u16;
                let limb_lo = val as u16 & 0xffff;
                trace[columns::VAL][i] = from_(val);
                trace[columns::LIMB_HI][i] = from_(limb_hi);
                trace[columns::LIMB_LO][i] = from_(limb_lo);
                trace[columns::CPU_FILTER][i] = F::ONE;
            }
            _ => {}
        }
    }

    trace[columns::FIXED_RANGE_CHECK_U16] =
        (0..RANGE_CHECK_U16_SIZE).map(|i| from_(i as u64)).collect();

    let (permuted_inputs, _) = permuted_cols(
        &trace[columns::LIMB_LO],
        &trace[columns::FIXED_RANGE_CHECK_U16],
    );

    trace[columns::LIMB_LO_PERMUTED] = permuted_inputs;

    let (permuted_inputs, permuted_table) = permuted_cols(
        &trace[columns::LIMB_HI],
        &trace[columns::FIXED_RANGE_CHECK_U16],
    );

    trace[columns::LIMB_HI_PERMUTED] = permuted_inputs;
    trace[columns::FIXED_RANGE_CHECK_U16_PERMUTED] = permuted_table;
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
                for i in 0..column.len() {
                    // Only the first two instructions are ADD, which require a range check
                    if i < 2 {
                        assert_eq!(column[i], F::ONE);
                    } else {
                        assert_eq!(column[i], F::ZERO);
                    }
                }
            }

            if idx == columns::VAL {
                for i in 0..column.len() {
                    match i {
                        // 100 + 100 = 200
                        0 => assert_eq!(column[i], GoldilocksField(200)),
                        // exit instruction
                        1 => assert_eq!(column[i], GoldilocksField(93)),
                        _ => assert_eq!(column[i], F::ZERO),
                    }
                }
            }
        }
    }
}
