use std::ops::Add;

use p3_field::AbstractField;

pub fn reduce_with_powers<V, E, I: IntoIterator<Item = V>>(terms: I, alpha: E) -> E
where
    V: Add<E, Output = E>,
    E: AbstractField,
    I::IntoIter: DoubleEndedIterator, {
    let mut sum = E::zero();
    for term in terms.into_iter().rev() {
        sum = term + alpha.clone() * sum;
    }
    sum
}
