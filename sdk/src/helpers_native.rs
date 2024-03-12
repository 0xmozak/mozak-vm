// This file contains code snippets used in native execution

use std::collections::HashMap;
use std::hash::Hash;

use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::{Poseidon2Hash, Poseidon2Permutation};
use plonky2::plonk::config::{GenericHashOut, Hasher};

/// Sort a given input vector (not in-place), returns
/// such sorted vector alongwith the mapping of each element
/// in original vector, called `hint`. Returns in the format
/// `(sorted_vec, hints)`.
///
/// # Examples
/// ```
/// let input: Vec<char>            = vec!['f', 'c', 'd', 'b', 'a', 'e'];
/// let expected_sorted: Vec<char>  = vec!['a', 'b', 'c', 'd', 'e', 'f'];
/// let expected_hints: Vec<usize>  = vec![ 5 ,  2 ,  3 ,  1 ,  0 ,  4 ];
///
/// let (sorted, hints) = sort_with_hints(input);
///
/// assert_eq!(expected_sorted, sorted);
/// assert_eq!(expected_hints, hints);
/// ```
#[allow(dead_code)]
#[allow(clippy::needless_pass_by_value)]
pub fn sort_with_hints<T, K>(input: Vec<T>) -> (Vec<T>, Vec<K>)
where
    T: Clone + Hash + Ord,
    K: From<usize> + Copy, {
    let sorted = {
        let mut clone = input.clone();
        clone.sort();
        clone
    };

    let mut element_index_map: HashMap<&T, K> = HashMap::with_capacity(input.len());
    for (i, elem) in sorted.iter().enumerate() {
        element_index_map.insert(elem, i.into());
    }

    let mut hints = Vec::with_capacity(input.len());
    for elem in &input {
        if let Some(index) = element_index_map.get(elem) {
            hints.push(*index);
        } else {
            panic!("cannot find elem in map!");
        }
    }

    (sorted, hints)
}

/// Hashes the input slice to `Poseidon2HashType`
pub fn poseidon2_hash(input: &[u8]) -> Poseidon2HashType {
    let mut padded_input = input.to_vec();
    padded_input.push(1);

    const RATE: usize = 8;

    padded_input.resize(padded_input.len().next_multiple_of(RATE), 0);
    let data_fields: Vec<GoldilocksField> = padded_input
        .iter()
        .map(|x| GoldilocksField::from_canonical_u8(*x))
        .collect();

    Poseidon2HashType(
        Poseidon2Hash::hash_no_pad(&data_fields)
            .to_bytes()
            .try_into()
            .expect("Output length does not match to DIGEST_BYTES"),
    )
}

#[cfg(test)]
mod tests {
    use super::sort_with_hints;

    #[test]
    #[rustfmt::skip]
    fn test_sort_with_hints() {
        let input: Vec<char>            = vec!['f', 'c', 'd', 'b', 'a', 'e'];
        let expected_sorted: Vec<char>  = vec!['a', 'b', 'c', 'd', 'e', 'f'];
        let expected_hints: Vec<usize>  = vec![ 5 ,  2 ,  3 ,  1 ,  0 ,  4 ];

        let (sorted, hints) = sort_with_hints(input);

        assert_eq!(expected_sorted, sorted);
        assert_eq!(expected_hints, hints);
    }
}
