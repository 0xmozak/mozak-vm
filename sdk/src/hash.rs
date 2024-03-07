use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::{Field, PrimeField64};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::plonk::config::Hasher;
type F = GoldilocksField;
type H = Poseidon2Hash;

pub fn hash(values: &[u8]) -> [u64; 4] {
    let values_as_field: Vec<F> = values.iter().map(|v| F::from_canonical_u8(*v)).collect();
    H::hash_no_pad(&values_as_field)
        .elements
        .map(|limb| F::to_canonical_u64(&limb))
}
