//! Implementation of the Halo2 lookup argument.
//!
//! References:
//! - [ZCash Halo2 lookup docs](https://zcash.github.io/halo2/design/proving-system/lookup.html)
//! - [ZK Meetup Seoul ECC X ZKS Deep dive on Halo2](https://www.youtube.com/watch?v=YlTt12s7vGE&t=5237s)

use std::cmp::Ordering;

use itertools::Itertools;
use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::{Field, PrimeField64};
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::{
    constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer},
    vars::{StarkEvaluationTargets, StarkEvaluationVars},
};

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

pub(crate) fn eval_lookups_circuit<
    F: RichField + Extendable<D>,
    const D: usize,
    const COLS: usize,
    const PUBLIC_INPUTS: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    vars: StarkEvaluationTargets<D, COLS, PUBLIC_INPUTS>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    col_permuted_input: usize,
    col_permuted_table: usize,
) {
    let local_perm_input = vars.local_values[col_permuted_input];
    let next_perm_table = vars.next_values[col_permuted_table];
    let next_perm_input = vars.next_values[col_permuted_input];

    // A "vertical" diff between the local and next permuted inputs.
    let diff_input_prev = builder.sub_extension(next_perm_input, local_perm_input);
    // A "horizontal" diff between the next permuted input and permuted table value.
    let diff_input_table = builder.sub_extension(next_perm_input, next_perm_table);

    let diff_product = builder.mul_extension(diff_input_prev, diff_input_table);
    yield_constr.constraint(builder, diff_product);

    // This is actually constraining the first row, as per the spec, since
    // `diff_input_table` is a diff of the next row's values. In the context of
    // `constraint_last_row`, the next row is the first row.
    yield_constr.constraint_last_row(builder, diff_input_table);
}

/// Given an input column and a table column, Prepares the permuted input column
/// `A'` and permuted table column `S'` used in the [Halo2 permutation
/// argument](https://zcash.github.io/halo2/design/proving-system/lookup.html).
///
/// # Returns
/// A tuple of the permuted input column, `A'`, and the permuted table column,
/// `S'`.
pub fn permute_cols<F: PrimeField64>(col_input: &[F], col_table: &[F]) -> (Vec<F>, Vec<F>) {
    let n = col_input.len();

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

    let mut unused_table_inds = Vec::with_capacity(n);
    let mut unused_table_vals = Vec::with_capacity(n);
    let mut col_table_permuted = vec![F::ZERO; n];
    let mut i = 0;
    let mut j = 0;
    while (j < n) && (i < n) {
        let input_val = col_input_sorted[i].to_noncanonical_u64();
        let table_val = col_table_sorted[j].to_noncanonical_u64();
        match input_val.cmp(&table_val) {
            // In the below tables, we ignore the original input column `col_input` (A),
            // and only care about `col_input_sorted` (A'), `col_table_permuted` (S'), and
            // `col_table_sorted` (S).
            //
            // -------------
            // | A'| S'| S |
            // |---|---|---|
            // | 4 | . | 3 | <- push 3 to `unused_table_vals` since
            // |   |   |   |    A' (col_input_sorted) > S (col_table_sorted)
            Ordering::Greater => {
                unused_table_vals.push(col_table_sorted[j]);
                j += 1;
            }

            // -------------
            // | A'| S'| S |    if `unused_table_vals` has some value, insert
            // |---|---|---|    into S' (col_table_permuted), else save its index to be
            // | 2 | . | 3 | <- populated later. It does not matter what is in S',
            // |   |   |   |    as long as it belongs in S (col_table_sorted).
            //                  This case also means that our lookup constraint later will
            //                  rely on the previous A' to be equal to the current A'
            //                  to hold (diff_input_prev = next_perm_input - local_perm_input).
            Ordering::Less => {
                if let Some(x) = unused_table_vals.pop() {
                    col_table_permuted[i] = x;
                } else {
                    unused_table_inds.push(i);
                }
                i += 1;
            }
            // -------------
            // | A'| S'| S |    if A' (col_input_sorted) == S (col_table_sorted),
            // |---|---|---|    insert into S' (col_table_permuted). This case also
            // | 2 | 2 | 2 | <- means that our lookup constraint holds,
            // |   |   |   |    since horizontally, diff_input_table = next_perm_input -
            //                  next_perm_table.
            Ordering::Equal => {
                col_table_permuted[i] = col_table_sorted[j];
                i += 1;
                j += 1;
            }
        }
    }

    unused_table_vals.extend_from_slice(&col_table_sorted[j..n]);
    unused_table_inds.extend(i..n);

    // Populate all the empty `S'` values found in the 2nd case above.
    for (ind, val) in unused_table_inds.into_iter().zip_eq(unused_table_vals) {
        col_table_permuted[ind] = val;
    }

    (col_input_sorted, col_table_permuted)
}

pub fn permute_cols_<F: PrimeField64>(col_input: &[F], col_table: &[F]) -> (Vec<F>, Vec<F>) {
    let n = col_input.len();

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

    let mut unused_table_inds = Vec::with_capacity(n);
    let mut unused_table_vals = Vec::with_capacity(n);
    let mut col_table_permuted = vec![F::ZERO; n];
    let mut i = 0;
    let mut j = 0;
    while (j < n) && (i < n) {
        let input_val = col_input_sorted[i].to_noncanonical_u64();
        let table_val = col_table_sorted[j].to_noncanonical_u64();
        match input_val.cmp(&table_val) {
            // In the below tables, we ignore the original input column `col_input` (A),
            // and only care about `col_input_sorted` (A'), `col_table_permuted` (S'), and
            // `col_table_sorted` (S).
            //
            // -------------
            // | A'| S'| S |
            // |---|---|---|
            // | 4 | . | 3 | <- push 3 to `unused_table_vals` since
            // |   |   |   |    A' (col_input_sorted) > S (col_table_sorted)
            Ordering::Greater => {
                unused_table_vals.push(col_table_sorted[j]);
                j += 1;
            }

            // -------------
            // | A'| S'| S |    if `unused_table_vals` has some value, insert
            // |---|---|---|    into S' (col_table_permuted), else save its index to be
            // | 2 | . | 3 | <- populated later. It does not matter what is in S',
            // |   |   |   |    as long as it belongs in S (col_table_sorted).
            //                  This case also means that our lookup constraint later will
            //                  rely on the previous A' to be equal to the current A'
            //                  to hold (diff_input_prev = next_perm_input - local_perm_input).
            Ordering::Less => {
                if let Some(x) = unused_table_vals.pop() {
                    col_table_permuted[i] = x;
                } else {
                    unused_table_inds.push(i);
                }
                i += 1;
            }
            // -------------
            // | A'| S'| S |    if A' (col_input_sorted) == S (col_table_sorted),
            // |---|---|---|    insert into S' (col_table_permuted). This case also
            // | 2 | 2 | 2 | <- means that our lookup constraint holds,
            // |   |   |   |    since horizontally, diff_input_table = next_perm_input -
            //                  next_perm_table.
            Ordering::Equal => {
                col_table_permuted[i] = col_table_sorted[j];
                i += 1;
                j += 1;
            }
        }
    }

    unused_table_vals.extend_from_slice(&col_table_sorted[j..n]);
    unused_table_inds.extend(i..n);

    // Populate all the empty `S'` values found in the 2nd case above.
    for (ind, val) in unused_table_inds.into_iter().zip_eq(unused_table_vals) {
        col_table_permuted[ind] = val;
    }

    (col_input_sorted, col_table_permuted)
}

#[cfg(test)]
mod test {
    use proptest::prelude::*;
    use plonky2::field::types::Field64;
    type F = plonky2::field::goldilocks_field::GoldilocksField;

    proptest! {

        #[test]
        fn oracle(col_input in any::<Vec<u64>>(), col_table in any::<Vec<u64>>())  {
            let col_input = col_input.into_iter().map(F::from_noncanonical_u64).collect::<Vec<_>>();
            let col_table = col_table.into_iter().map(F::from_noncanonical_u64).collect::<Vec<_>>();
            // pub fn permute_cols_<F: PrimeField64>(col_input: &[F], col_table: &[F]) -> (Vec<F>, Vec<F>) {
            let old = super::permute_cols::<F>(&col_input, &col_table);
            let new = super::permute_cols_::<F>(&col_input, &col_table);
            prop_assert!(old == new);
        }
    }
}
