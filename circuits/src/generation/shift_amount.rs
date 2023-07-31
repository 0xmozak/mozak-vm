use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::{self as cpu_cols};
use crate::lookup::permute_cols;
use crate::shift_amount::columns::{Executed, ShiftAmountView, FIXED_SHAMT_RANGE};

/// Returns the rows for shift instructions.
#[must_use]
pub fn filter_shift_trace(step_rows: &[Row]) -> Vec<usize> {
    step_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            matches!(
                row.state.current_instruction().op,
                Op::SLL | Op::SRL | Op::SRA
            )
        })
        .map(|(i, _row)| i)
        .collect()
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_shift_amount_trace<F: RichField>(
    step_rows: &[Row],
    cpu_trace: &[Vec<F>; cpu_cols::NUM_CPU_COLS],
) -> Vec<ShiftAmountView<F>> {
    let filtered_step_rows = filter_shift_trace(step_rows);
    let mut trace: Vec<ShiftAmountView<F>> = vec![];
    let trace_len = filtered_step_rows.len().max(FIXED_SHAMT_RANGE.end.into());
    trace.resize(trace_len, ShiftAmountView {
        executed: Executed {
            shamt: F::ZERO,
            multiplier: F::ONE,
        },
        ..Default::default()
    });
    for (i, clk) in filtered_step_rows.iter().enumerate() {
        trace[i].is_executed = F::ONE;
        trace[i].executed.shamt = cpu_trace[cpu_cols::MAP.powers_of_2_in][*clk];
        trace[i].executed.multiplier = cpu_trace[cpu_cols::MAP.powers_of_2_out][*clk];
    }
    for (i, value) in trace.iter_mut().enumerate().take(trace_len) {
        if i < FIXED_SHAMT_RANGE.end.into() {
            value.fixed_shamt = F::from_canonical_usize(i);
            value.fixed_multiplier = F::from_canonical_usize(1 << i);
        } else {
            value.fixed_shamt = F::from_canonical_usize(31);
            value.fixed_multiplier = F::from_canonical_usize(1 << 31);
        }
    }
    let shamt: Vec<F> = trace.iter().map(|v| v.executed.shamt).collect();
    let fixed_shamt: Vec<F> = trace.iter().map(|v| v.fixed_shamt).collect();
    let (shamt_permuted, fixed_shamt_permuted) = permute_cols(&shamt, &fixed_shamt);
    assert!(shamt_permuted.len() == trace_len);
    assert!(fixed_shamt_permuted.len() == trace_len);
    for (i, (p, v)) in shamt_permuted
        .iter()
        .zip(fixed_shamt_permuted.iter())
        .enumerate()
    {
        trace[i].shamt_permuted = *p;
        trace[i].fixed_shamt_permuted = *v;
    }
    let multiplier: Vec<F> = trace.iter().map(|v| v.executed.multiplier).collect();
    let fixed_multiplier: Vec<F> = trace.iter().map(|v| v.fixed_multiplier).collect();
    let (multiplier_permuted, fixed_multiplier_permuted) =
        permute_cols(&multiplier, &fixed_multiplier);
    assert!(multiplier_permuted.len() == trace_len);
    assert!(fixed_multiplier_permuted.len() == trace_len);
    for (i, (p, v)) in multiplier_permuted
        .iter()
        .zip(fixed_multiplier_permuted.iter())
        .enumerate()
    {
        trace[i].multiplier_permuted = *p;
        trace[i].fixed_multiplier_permuted = *v;
    }
    trace
}
