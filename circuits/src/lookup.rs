//! Implementation of the Halo2 lookup argument.
//!
//! References:
//! - [ZCash Halo2 lookup docs](https://zcash.github.io/halo2/design/proving-system/lookup.html)
//! - [ZK Meetup Seoul ECC X ZKS Deep dive on Halo2](https://www.youtube.com/watch?v=YlTt12s7vGE&t=5237s)

use std::collections::VecDeque;

use itertools::Itertools;
use plonky2::field::packed::PackedField;
use plonky2::field::types::{Field, PrimeField64};
use starky::constraint_consumer::ConstraintConsumer;
use starky::vars::StarkEvaluationVars;

pub(crate) fn eval_lookups<
    F: Field,
    P: PackedField<Scalar = F>,
    const COLS: usize,
    const PUBLIC_INPUTS: usize,
>(
    vars: StarkEvaluationVars<F, P, COLS, PUBLIC_INPUTS>,
    yield_constr: &mut ConstraintConsumer<P>,
    col_permuted_input: usize,
    col_permuted_table: usize,
) {
    let local_perm_input = vars.local_values[col_permuted_input];
    let next_perm_table = vars.next_values[col_permuted_table];
    let next_perm_input = vars.next_values[col_permuted_input];

    // A "vertical" diff between the local and next permuted inputs.
    let diff_input_prev = next_perm_input - local_perm_input;
    // A "horizontal" diff between the next permuted input and permuted table value.
    let diff_input_table = next_perm_input - next_perm_table;

    yield_constr.constraint(diff_input_prev * diff_input_table);

    // This is actually constraining the first row, as per the spec, since
    // `diff_input_table` is a diff of the next row's values. In the context of
    // `constraint_last_row`, the next row is the first row.
    yield_constr.constraint_last_row(diff_input_table);
}

/// Given an input column and a table column, Prepares the permuted input column
/// `A'` and permuted table column `S'` used in the [Halo2 permutation
/// argument](https://zcash.github.io/halo2/design/proving-system/lookup.html).
///
/// # Returns
/// A tuple of the permuted input column, `A'`, and the permuted table column,
/// `S'`.
///
/// # Panics
/// Panics if there are unused values or indices left, since in the lookup
/// protocol the permuted table column must be a permutation of the original
/// column, so any unused values or unfilled spots would indicate a logic bug.
pub fn permute_cols<F: PrimeField64>(col_input: &[F], col_table: &[F]) -> (Vec<F>, Vec<F>) {
    // The permuted inputs do not have to be ordered, but we found that sorting was
    // faster than hash-based grouping. We also sort the table, as this helps us
    // identify "unused" table elements efficiently.

    // To compare elements, e.g. for sorting, we first need them in canonical form.
    // It would be wasteful to canonicalize in each comparison, as a single
    // element may be involved in many comparisons. So we will canonicalize once
    // upfront, then use `to_noncanonical_u64` when comparing elements.
    let col_input_sorted = col_input
        .iter()
        .map(PrimeField64::to_canonical)
        .sorted_unstable_by_key(PrimeField64::to_noncanonical_u64)
        .collect_vec();
    let col_table_sorted = col_table
        .iter()
        .map(PrimeField64::to_canonical)
        .sorted_unstable_by_key(PrimeField64::to_noncanonical_u64)
        .collect_vec();

    let mut unused_table_inds = VecDeque::new();
    let mut unused_table_vals = VecDeque::new();
    let mut col_table_permuted: Vec<Option<F>> = vec![];
    col_input_sorted
        .iter()
        .merge_join_by(col_table_sorted.iter(), |input, target| {
            input
                .to_noncanonical_u64()
                .cmp(&target.to_noncanonical_u64())
        })
        .for_each(|y| match y {
            itertools::EitherOrBoth::Left(_) => {
                if let Some(x) = unused_table_vals.pop_front() {
                    col_table_permuted.push(Some(x));
                } else {
                    unused_table_inds.push_back(col_table_permuted.len());
                    // Here, we push None as a placeholder to be replaced later.
                    col_table_permuted.push(None);
                }
            }
            itertools::EitherOrBoth::Both(_, b) => col_table_permuted.push(Some(*b)),
            itertools::EitherOrBoth::Right(b) => {
                if let Some(i) = unused_table_inds.pop_front() {
                    // Replace the placeholder.
                    col_table_permuted[i] = Some(*b);
                } else {
                    unused_table_vals.push_back(*b);
                }
            }
        });
    assert_eq!(unused_table_inds.len(), 0);
    assert_eq!(unused_table_vals.len(), 0);

    // Nice trick to unwrap the `Some<F>`s safely:
    // https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flatten
    let col_table_permuted: Vec<F> = col_table_permuted.into_iter().flatten().collect();

    // If there were placeholder `None`s remaining, this would fail.
    assert_eq!(col_table_permuted.len(), col_input_sorted.len());

    (col_input_sorted, col_table_permuted)
}

#[cfg(test)]
mod tests {
    use plonky2::field::types::PrimeField64;
    use proptest::prelude::*;

    use super::*;
    type F = plonky2::field::goldilocks_field::GoldilocksField;

    proptest! {
        #[test]
        fn test_permute_cols(value in any::<Vec<u64>>())  {
            let col_input  = value.iter().map(|i| F::from_noncanonical_u64(*i)).collect::<Vec<_>>();
            let col_table = value.iter().map(|i| F::from_noncanonical_u64(*i)).collect::<Vec<_>>();

            let mut col_table_u64: Vec<_> = col_table.iter().map(F::to_noncanonical_u64).collect();
            let mut col_input_u64: Vec<_> = col_input.iter().map(F::to_noncanonical_u64).collect();

            let (col_input_sorted, col_table_permuted) = permute_cols::<F>(&col_input, &col_table);

            col_table_u64.sort_unstable();
            col_input_u64.sort_unstable();

            let col_input_sorted_u64: Vec<_> = col_input_sorted.iter().map(F::to_noncanonical_u64).collect();
            let col_table_permuted_u64: Vec<_> = col_table_permuted.iter().map(F::to_noncanonical_u64).collect();

            // We want to be sure that the result table column
            // is actually a permutation of the input table column.
            // Checking the input column may actually not be necessary
            // since all we do is sort it.
            assert_eq!(col_table_u64, col_table_permuted_u64);
            assert_eq!(col_input_u64, col_input_sorted_u64);
        }
    }
}

// PROPTEST_MAX_SHRINK_ITERS=1000000
