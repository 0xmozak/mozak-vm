use plonky2::field::packed::PackedField;
use plonky2::field::{polynomial::PolynomialValues, types::Field};
use starky::constraint_consumer::ConstraintConsumer;

pub fn trace_to_poly_values<F: Field, const COLUMNS: usize>(
    trace: [Vec<F>; COLUMNS],
) -> Vec<PolynomialValues<F>> {
    trace.into_iter().map(PolynomialValues::new).collect()
}

/// Selector of opcode, builtins and halt should be one-hot encoded.
///
/// Ie exactly one of them should be by 1, all others by 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
pub fn opcode_one_hot<P: PackedField>(
    lv: &[P],
    num_cols: usize,
    start_col: usize,
    end_col: usize,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Ensure the input vector has enough columns
    assert!(lv.len() >= num_cols, "Input vector is too short");
    assert!(
        start_col < num_cols,
        "Starting column index is out of bounds"
    );
    assert!(end_col < num_cols, "Ending column index is out of bounds");

    let op_selectors = [lv[start_col], lv[end_col]];

    op_selectors
        .into_iter()
        .for_each(|s| yield_constr.constraint(s * (P::ONES - s)));

    // Only one opcode selector enabled.
    let sum_s_op: P = op_selectors.into_iter().sum();
    yield_constr.constraint(P::ONES - sum_s_op);
}
