use mozak_vm::trace::RangeCheckRow;
use plonky2::hash::hash_types::RichField;

use crate::lookup::permuted_cols;
use crate::rangecheck::columns;
use crate::utils::from_;

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

pub fn generate_fixed_rangecheck_table<F: RichField>(mut trace: Vec<Vec<F>>) -> Vec<Vec<F>> {
    trace[columns::FIXED_RANGE_CHECK_U16] = (0..columns::RANGE_CHECK_U16_SIZE)
        .map(|i| from_(i as u64))
        .collect();

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

    trace
}

pub fn generate_rangecheck_trace<F: RichField>(
    rangecheck_rows: &[RangeCheckRow],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let trace_len = rangecheck_rows.len();
    let max_trace_len = trace_len.max(columns::RANGE_CHECK_U16_SIZE);
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; max_trace_len]; columns::NUM_RC_COLS];
    for (i, r) in rangecheck_rows.iter().enumerate() {
        trace[columns::VAL][i] = from_(r.val);
        trace[columns::LIMB_HI][i] = from_(r.limb_hi);
        trace[columns::LIMB_LO][i] = from_(r.limb_lo);
        trace[columns::CPU_FILTER][i] = from_(r.filter_cpu);
    }

    let trace = generate_fixed_rangecheck_table(trace);
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
    use mozak_vm::{test_utils::simple_test, trace::RangeCheckRow};

    #[test]
    fn test_add_instruction_inserts_rangecheck() {
        let (_, state) = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            &[(6, 100), (7, 100)],
        );

        println!("state: {:?}", state);
        assert_eq!(state.get_register_value(5), 100 + 100);
        let row = state.trace.rangecheck_column[0];
        let expected_row = RangeCheckRow {
            val: 200,
            limb_lo: 200,
            limb_hi: 0,
            filter_cpu: 1,
        };

        assert_eq!(state.trace.rangecheck_column.len(), 1);
        assert_eq!(row, expected_row);
    }
}
