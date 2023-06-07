use plonky2::field::{polynomial::PolynomialValues, types::Field};

pub fn trace_to_poly_values<F: Field, const COLUMNS: usize>(
    trace: [Vec<F>; COLUMNS],
) -> Vec<PolynomialValues<F>> {
    trace
        .into_iter()
        .map(|row| PolynomialValues::new(row))
        .collect()
}
