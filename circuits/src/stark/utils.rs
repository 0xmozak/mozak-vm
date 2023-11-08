use itertools::{Itertools, MergeBy};
use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::util::transpose;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

/// Ensure an expression only takes on values 0 or 1.
/// This doubles the degree of the provided expression `x`,
/// so as long as we are targeting degree <= 3,
/// this should only be called with at most linear expressions.
pub fn is_binary<P: PackedField>(yield_constr: &mut ConstraintConsumer<P>, x: P) {
    yield_constr.constraint(x * (P::ONES - x));
}

pub fn is_binary_ext_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    x: ExtensionTarget<D>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let one = builder.one_extension();
    let x_sub_one = builder.sub_extension(one, x);
    let x_mul_x_sub_one = builder.mul_extension(x, x_sub_one);
    yield_constr.constraint(builder, x_mul_x_sub_one);
}

#[must_use]
pub fn trace_to_poly_values<F: Field, Grid: IntoIterator<Item = Vec<F>>>(
    trace: Grid,
) -> Vec<PolynomialValues<F>> {
    trace.into_iter().map(PolynomialValues::new).collect()
}

/// Transform a given row-major trace to a column-major trace by flipping it over its diagonal.
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
