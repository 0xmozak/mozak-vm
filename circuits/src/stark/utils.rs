use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;

pub fn trace_to_poly_values<F: Field, const COLUMNS: usize>(
    trace: [Vec<F>; COLUMNS],
) -> Vec<PolynomialValues<F>> {
    trace.into_iter().map(PolynomialValues::new).collect()
}
