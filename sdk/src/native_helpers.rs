// This file contains code snippets used in native execution

use std::collections::HashMap;
use std::hash::Hash;

/// Sort a given input vector (not in-place), returns
/// such sorted vector alongwith the mapping of each element
/// in original vector, called `hint`. Returns in the format
/// `(sorted_vec, hints)`.
///
/// # Examples
/// ```
/// let input: Vec<char>            = vec!['f', 'c', 'd', 'b', 'a', 'e'];
/// let expected_sorted: Vec<char>  = vec!['a', 'b', 'c', 'd', 'e', 'f'];
/// let expected_hints: Vec<usize>  = vec![  4,   3,   1,   2,   5,   0];
///
/// let (sorted, hints) = sort_with_hints(input);
///
/// assert_eq!(expected_sorted, sorted);
/// assert_eq!(expected_hints, hints);
/// ```
pub fn sort_with_hints<T, K>(input: &[T]) -> (Vec<T>, Vec<K>)
where
    T: Clone + Hash + Ord,
    K: From<usize> + Copy, {
    let sorted = {
        let mut clone = input.to_owned();
        clone.sort();
        clone
    };

    let mut element_index_map: HashMap<&T, K> = HashMap::with_capacity(input.len());
    for (i, elem) in input.iter().enumerate() {
        element_index_map.insert(elem, i.into());
    }

    let mut hints = Vec::with_capacity(input.len());
    for elem in &sorted {
        if let Some(index) = element_index_map.get(elem) {
            hints.push(*index);
        } else {
            panic!("cannot find elem in map!");
        }
    }

    (sorted, hints)
}

#[cfg(test)]
mod tests {
    use super::sort_with_hints;

    #[test]
    #[rustfmt::skip]
    fn test_sort_with_hints() {
        let input: Vec<char>            = vec!['f', 'c', 'd', 'b', 'a', 'e'];
        let expected_sorted: Vec<char>  = vec!['a', 'b', 'c', 'd', 'e', 'f'];
        let expected_hints: Vec<usize>  = vec![  4,   3,   1,   2,   5,   0];

        let (sorted, hints) = sort_with_hints(&input);

        assert_eq!(expected_sorted, sorted);
        assert_eq!(expected_hints, hints);
    }
}
