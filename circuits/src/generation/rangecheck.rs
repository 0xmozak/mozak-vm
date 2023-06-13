use plonky2::hash::hash_types::RichField;

use crate::lookup::permuted_cols;
use crate::rangecheck::columns;
use crate::utils::from_;

#[derive(Debug, Clone, Copy)]
pub struct RangeCheckRow {
    pub val: u32,
    pub limb_lo: u16,
    pub limb_hi: u16,
    pub filter_cpu: u32,
}

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
        .map(|i| F::from_canonical_usize(i))
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
    let ext_trace_len = if !max_trace_len.is_power_of_two() || max_trace_len < 2 {
        if max_trace_len < 2 {
            2
        } else {
            max_trace_len.next_power_of_two()
        }
    } else {
        max_trace_len
    };
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; columns::NUM_RC_COLS];
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
