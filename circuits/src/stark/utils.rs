use itertools::{Itertools, MergeBy};
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::util::transpose;

#[must_use]
pub fn trace_to_poly_values<F: Field, Grid: IntoIterator<Item = Vec<F>>>(
    trace: Grid,
) -> Vec<PolynomialValues<F>> {
    trace.into_iter().map(PolynomialValues::new).collect()
}

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
#[must_use]
pub fn trace_rows_to_poly_values<F: Field, Row: IntoIterator<Item = F>>(
    trace_rows: Vec<Row>,
) -> Vec<PolynomialValues<F>> {
    trace_to_poly_values(transpose_trace(trace_rows))
}

pub fn merge_by_key<Iter, J, F, Key>(
    iter: Iter,
    other: J,
    mut key: F,
) -> MergeBy<Iter, J::IntoIter, impl FnMut(&Iter::Item, &Iter::Item) -> bool>
where
    Iter: Sized + Iterator,
    J: IntoIterator<Item = Iter::Item>,
    F: FnMut(&Iter::Item) -> Key,
    Key: PartialOrd, {
    iter.merge_by(other, move |x, y| key(x) < key(y))
}
