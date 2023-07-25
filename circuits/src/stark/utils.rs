use itertools::Itertools;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::util::transpose;

pub fn trace_to_poly_values<F: Field, Grid: IntoIterator<Item = Vec<F>>>(
    trace: Grid,
) -> Vec<PolynomialValues<F>> {
    trace.into_iter().map(PolynomialValues::new).collect()
}

/// A helper function to transpose a row-wise trace and put it in the format
/// that `prove` expects.
#[must_use]
pub fn trace_rows_to_poly_values<F: Field, Row: IntoIterator<Item = F>>(
    trace_rows: Vec<Row>,
) -> Vec<PolynomialValues<F>> {
    let trace_row_vecs = trace_rows
        .into_iter()
        .map(|row| row.into_iter().collect_vec())
        .collect_vec();
    trace_to_poly_values(transpose(&trace_row_vecs))
}
