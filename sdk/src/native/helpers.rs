// This file contains code snippets used in native execution

use std::collections::HashMap;
use std::hash::Hash;

use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::hash::poseidon2::Poseidon2Hash as Plonky2Poseidon2Hash;
use plonky2::plonk::config::{GenericHashOut, Hasher};

use crate::common::types::{Poseidon2Hash, ProgramIdentifier, SystemTape};

/// Represents a stack for call contexts during native execution.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityStack(Vec<ProgramIdentifier>);

impl IdentityStack {
    pub fn add_identity(&mut self, id: ProgramIdentifier) { self.0.push(id); }

    pub fn top_identity(&self) -> ProgramIdentifier { self.0.last().copied().unwrap_or_default() }

    pub fn rm_identity(&mut self) { self.0.truncate(self.0.len().saturating_sub(1)); }
}

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

/// Hashes the input slice to `Poseidon2Hash`
pub fn poseidon2_hash(input: &[u8]) -> Poseidon2Hash {
    const RATE: usize = 8;
    let mut padded_input = input.to_vec();
    padded_input.push(1);

    padded_input.resize(padded_input.len().next_multiple_of(RATE), 0);
    let data_fields: Vec<GoldilocksField> = padded_input
        .iter()
        .map(|x| GoldilocksField::from_canonical_u8(*x))
        .collect();

    Poseidon2Hash(
        Plonky2Poseidon2Hash::hash_no_pad(&data_fields)
            .to_bytes()
            .try_into()
            .expect("Output length does not match to DIGEST_BYTES"),
    )
}

/// Writes a byte slice to a given file
fn write_to_file(file_path: &str, content: &[u8]) {
    use std::io::Write;
    let path = std::path::Path::new(file_path);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(content).unwrap();
}

/// Dumps a copy of `SYSTEM_TAPE` to disk, serialized
/// via `serde_json` as well as in rust debug file format
/// if opted for. Extension of `.tape` is used for serialized
/// formed of tape on disk, `.tape_debug` will be used for
/// debug tape on disk. Prior to dumping on disk, a pre-processor
/// runs on `SYSTEM_TAPE` as needed.
#[allow(dead_code)]
pub fn dump_system_tape(
    file_template: &str,
    is_debug_tape_required: bool,
    pre_processor: Option<impl Fn(SystemTape) -> SystemTape>,
) {
    let mut tape_clone = unsafe {
        crate::common::system::SYSTEM_TAPE.clone() // .clone() removes `Lazy{}`
    };

    if let Some(pre_processor) = pre_processor {
        tape_clone = pre_processor(tape_clone);
    }

    if is_debug_tape_required {
        write_to_file(
            &(file_template.to_string() + ".tape_debug"),
            &format!("{tape_clone:#?}").into_bytes(),
        );
    }

    write_to_file(
        &(file_template.to_string() + ".tape"),
        &serde_json::to_string_pretty(&tape_clone)
            .unwrap()
            .into_bytes(),
    );
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

        let (sorted, hints) = sort_with_hints::<char, usize>(input);

        assert_eq!(expected_sorted, sorted);
        assert_eq!(expected_hints, hints);
    }
}
