use itertools::Itertools;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::util::transpose;

#[must_use]
pub fn trace_to_poly_values<F: Field, Grid: IntoIterator<Item = Vec<F>>>(
    trace: Grid,
) -> Vec<PolynomialValues<F>> {
    trace.into_iter().map(PolynomialValues::new).collect()
}

/// Transform a given row-major trace to a column-major trace by flipping it
/// over its diagonal.
#[must_use]
pub fn transpose_trace<F: Field, Row: IntoIterator<Item = F>>(trace_rows: Vec<Row>) -> Vec<Vec<F>> {
    transpose(
        &trace_rows
            .into_iter()
            .map(|row| row.into_iter().collect_vec())
            .collect_vec(),
    )
}

/// A helper function to transpose a row-wise trace and put it in the format
/// that `prove` expects.
/// Intepret a row trace as a polynomial
#[must_use]
pub fn trace_rows_to_poly_values<F: Field, Row: IntoIterator<Item = F>>(
    trace_rows: Vec<Row>,
) -> Vec<PolynomialValues<F>> {
    trace_to_poly_values(transpose_trace(trace_rows))
}
