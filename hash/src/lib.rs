#![feature(const_float_bits_conv)]
use std::fmt::Debug;

use itertools::Itertools;
use plonky2::field::extension::quadratic::QuadraticExtension;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::hash::hashing::{compress, hash_n_to_hash_no_pad, PlonkyPermutation};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::plonk::config::{GenericConfig, Hasher};
use serde::{Deserialize, Serialize};

/// Configuration using Poseidon over the Goldilocks field.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToyGoldilocksConfig;
impl GenericConfig<2> for ToyGoldilocksConfig {
    type F = GoldilocksField;
    type FE = QuadraticExtension<Self::F>;
    type Hasher = ToyHash<Self::F>;
    type InnerHasher = PoseidonHash;
}

pub const SPONGE_RATE: usize = 8;
pub const SPONGE_CAPACITY: usize = 4;
pub const SPONGE_WIDTH: usize = SPONGE_RATE + SPONGE_CAPACITY;

#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct ToyPermutation<T> {
    state: [T; SPONGE_WIDTH],
}

impl<T: Eq> Eq for ToyPermutation<T> {}
impl<T> AsRef<[T]> for ToyPermutation<T> {
    fn as_ref(&self) -> &[T] { &self.state }
}

impl<F: Copy + Debug + Default + Eq + Send + Sync + RichField> PlonkyPermutation<F>
    for ToyPermutation<F>
{
    const RATE: usize = SPONGE_RATE;
    const WIDTH: usize = SPONGE_WIDTH;

    fn new<I: IntoIterator<Item = F>>(elts: I) -> Self {
        let mut perm = Self {
            state: [F::default(); SPONGE_WIDTH],
        };
        perm.set_from_iter(elts, 0);
        perm
    }

    fn set_elt(&mut self, elt: F, idx: usize) { self.state[idx] = elt; }

    fn set_from_slice(&mut self, elts: &[F], start_idx: usize) {
        let begin = start_idx;
        let end = start_idx + elts.len();
        self.state[begin..end].copy_from_slice(elts);
    }

    fn set_from_iter<I: IntoIterator<Item = F>>(&mut self, elts: I, start_idx: usize) {
        for (s, e) in self.state[start_idx..].iter_mut().zip(elts) {
            *s = e;
        }
    }

    fn permute(&mut self) {
        self.state = self
            .state
            .into_iter()
            .circular_tuple_windows()
            .map(|(a, b)| a + b + b)
            .collect_vec()
            .try_into()
            .unwrap();
    }

    fn squeeze(&self) -> &[F] { &self.state[..Self::RATE] }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ToyHash<F>(F);
impl<F: RichField> Hasher<F> for ToyHash<F> {
    type Hash = HashOut<F>;
    type Permutation = ToyPermutation<F>;

    const HASH_SIZE: usize = 4;

    fn hash_no_pad(input: &[F]) -> Self::Hash {
        hash_n_to_hash_no_pad::<F, Self::Permutation>(input)
    }

    fn two_to_one(left: Self::Hash, right: Self::Hash) -> Self::Hash {
        compress::<F, Self::Permutation>(left, right)
    }
}
