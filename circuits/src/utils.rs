use plonky2::field::types::Field;

// TODO(Matthias): We can convert via u64, once https://github.com/mir-protocol/plonky2/pull/1092 has landed.
pub fn from_<X, F: Field>(x: X) -> F
where
    u128: From<X>,
{
    Field::from_noncanonical_u128(u128::from(x))
}
