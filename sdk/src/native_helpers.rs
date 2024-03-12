// This file contains code snippets used in native execution

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
pub fn sort_with_hints<T, K>(input: Vec<T>) -> (Vec<T>, Vec<K>)
where
    T: Ord,
    K: From<usize> + Copy, {
    let mut indexed_values: Vec<(T, usize)> = input.into_iter().zip(0..).collect::<Vec<_>>();
    indexed_values.sort();
    let (sorted, hints) : (_, Vec<_>) = indexed_values.into_iter().unzip();
    (sorted, hints.into_iter().map(K::from).collect())
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
