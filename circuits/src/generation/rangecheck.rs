use itertools::chain;
use plonky2::hash::hash_types::RichField;

use crate::columns_view::NumberOfColumns;
use crate::cpu::columns::CpuState;
use crate::lookup::permute_cols;
use crate::rangecheck::columns::{
    InputColumnsView, RangeCheckColumnsView, U16InnerLookupColumnsView, MAP,
};
use crate::stark::utils::transpose_trace;

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16;

/// Pad the input trace table to the size of 2^k rows in
/// preparation for the Halo2 lookup argument.
///
/// Note that by right the column to be checked (A) and the fixed column (S)
/// have to be extended by dummy values known to be in the fixed column if they
/// are not of size 2^k, but because our fixed column is a range from 0..2^16-1,
/// initializing our trace to all [`F::ZERO`]s takes care of this step by
/// default.
#[must_use]
fn pad_input_trace<F: RichField>(mut trace: Vec<InputColumnsView<F>>) -> Vec<InputColumnsView<F>> {
    let len = trace.len().max(RANGE_CHECK_U16_SIZE).next_power_of_two();

    trace.resize(len, InputColumnsView::default());

    trace
}

/// Converts a [`RichField`] value into 2 u16 limbs represented in
/// [`RichField`].
fn limbs_from_field<F: RichField>(value: F) -> (F, F) {
    let value =
        u32::try_from(value.to_canonical_u64()).expect("casting dst value to u32 should succeed");
    (
        F::from_canonical_u32(value >> 16),
        F::from_canonical_u32(value & 0xffff),
    )
}

/// The main driver for rangecheck trace generation. Generates the input trace
/// and the fixed trace and puts them together to form a complete range check
/// trace.
pub fn generate_rangecheck_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
) -> RangeCheckColumnsView<Vec<F>> {
    let mut input = transpose_trace(generate_input_trace(cpu_trace));
    let fixed = generate_fixed_trace(&mut input);

    chain!(input, fixed).collect()
}

#[must_use]
/// Generates the fixed view of the rangecheck trace involved in the inner table
/// lookup argument.
///
/// This view contains the fixed range 0..2^16-1, the permuted table columns
/// and the permuted limbs of the values to be range checked.
///
/// As such, this view is self contained in the sense that it is generated from
/// 2 columns:
/// 1) the column containing values to be range checked, and
/// 2) the column containing the fixed range.
pub fn generate_fixed_trace<F: RichField>(trace: &mut Vec<Vec<F>>) -> Vec<Vec<F>> {
    let mut fixed_trace: Vec<Vec<F>> =
        vec![vec![]; U16InnerLookupColumnsView::<()>::NUMBER_OF_COLUMNS];

    let len = trace[MAP.input.u32_value]
        .len()
        .max(RANGE_CHECK_U16_SIZE)
        .next_power_of_two();

    fixed_trace
        .iter_mut()
        .for_each(move |c| c.resize(len, F::ZERO));

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    fixed_trace[MAP.permuted.fixed_range - trace.len()] = (0..RANGE_CHECK_U16_SIZE as u64)
        .map(F::from_noncanonical_u64)
        .collect();

    fixed_trace[MAP.permuted.fixed_range - trace.len()]
        .resize(len, F::from_canonical_u64(u64::from(u16::MAX)));

    // This permutation is done in accordance to the [Halo2 lookup argument
    // spec](https://zcash.github.io/halo2/design/proving-system/lookup.html)
    let (col_input_permuted, col_table_permuted) = permute_cols(
        &trace[MAP.input.limb_lo],
        &fixed_trace[MAP.permuted.fixed_range - trace.len()],
    );

    // We need a column for the lower limb.
    fixed_trace[MAP.permuted.limb_lo_permuted - trace.len()] = col_input_permuted;
    fixed_trace[MAP.permuted.fixed_range_permuted_lo - trace.len()] = col_table_permuted;

    let (col_input_permuted, col_table_permuted) = permute_cols(
        &trace[MAP.input.limb_hi],
        &fixed_trace[MAP.permuted.fixed_range - trace.len()],
    );

    // And we also need a column for the upper limb.
    fixed_trace[MAP.permuted.limb_hi_permuted - trace.len()] = col_input_permuted;
    fixed_trace[MAP.permuted.fixed_range_permuted_hi - trace.len()] = col_table_permuted;

    fixed_trace
}

/// Fill the trace table with inputs from other traces for range checks, used in
/// building a `RangeCheckStark` proof.
///
/// # Panics
///
/// Panics if:
/// 1. conversion of `dst_val` from u32 to u16 fails when splitting into limbs,
/// 2. trace width does not match the number of columns.
#[must_use]
pub fn generate_input_trace<F: RichField>(cpu_trace: &[CpuState<F>]) -> Vec<InputColumnsView<F>> {
    let mut trace: Vec<InputColumnsView<F>> = vec![];

    for cpu_row in cpu_trace {
        if cpu_row.inst.ops.add.is_one() {
            let (limb_hi, limb_lo) = limbs_from_field(cpu_row.dst_value);

            let row = InputColumnsView {
                u32_value: cpu_row.dst_value,
                limb_hi,
                limb_lo,
                cpu_filter: F::ONE,
            };

            trace.push(row);
        }
    }

    trace = pad_input_trace(trace);

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
        let trace = generate_rangecheck_trace::<F>(&cpu_rows);

        // Check values that we are interested in
        assert_eq!(trace[MAP.input.cpu_filter][0], F::ONE);
        assert_eq!(trace[MAP.input.cpu_filter][1], F::ONE);
        assert_eq!(trace[MAP.input.u32_value][0], GoldilocksField(0x0001_fffe));
        assert_eq!(trace[MAP.input.u32_value][1], GoldilocksField(93));
        assert_eq!(trace[MAP.input.limb_hi][0], GoldilocksField(0x0001));
        assert_eq!(trace[MAP.input.limb_lo][0], GoldilocksField(0xfffe));
        assert_eq!(trace[MAP.input.limb_lo][1], GoldilocksField(93));

        // Ensure rest of trace is zeroed out
        for cpu_filter in &trace[MAP.input.cpu_filter][2..] {
            assert_eq!(cpu_filter, &F::ZERO);
        }
        for value in &trace[MAP.input.u32_value][2..] {
            assert_eq!(value, &F::ZERO);
        }
        for limb_hi in &trace[MAP.input.limb_hi][1..] {
            assert_eq!(limb_hi, &F::ZERO);
        }
        for limb_lo in &trace[MAP.input.limb_lo][2..] {
            assert_eq!(limb_lo, &F::ZERO);
        }
    }
}
