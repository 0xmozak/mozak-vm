//! To make a Subtable of given stark table public, we follow similar idea
//! used in CTL. The basic idea is to "compress" the subtable into a single
//! value which the verifier can construct on its own, and compare against.
//! Grand product argument, combined with randomness is a good option in
//! such situation. We use its equivalent, Logarithmic derivative approach
//! instead, especially because it lets us combine it with CTL proof system
//! which we have already. Essentially, given a subtable, we `combine` its rows
//! and maintain its running sum of inverses as values of z polynomial. The
//! opening of this z polynomial would be the "compressed" value, and can
//! be reproduced on verifer's end. We can also reuse the challenges used for
//! CTL to `combine`, since the procedure is preceded by commitment to trace
//! polynomials already
#![allow(clippy::module_name_repetitions)]
use itertools::{iproduct, Itertools};
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::reduce_with_powers_circuit;

use crate::cross_table_lookup::{partial_sums, CtlData, CtlZData};
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};

/// Specifies a Subtable with `table.columns` and `table.filter_column`
/// which the prover wants to make public. We include `num_rows` since
/// it cannot be computed from `table` alone.
#[derive(Clone, Debug)]
pub struct PublicSubTable {
    pub table: Table,
    pub num_rows: usize,
}
/// Actual values, as field elements, of the entries
/// of `PublicSubTable`
pub type PublicSubTableValues<F> = Vec<Vec<F>>;
/// Plonky2 target version of `PublicSubTableValuesTarget`
pub type PublicSubTableValuesTarget = Vec<Vec<Target>>;
impl PublicSubTable {
    #[must_use]
    pub fn num_zs(public_sub_tables: &[Self], table: TableKind, num_challenges: usize) -> usize {
        public_sub_tables
            .iter()
            .filter(|twc| twc.table.kind == table)
            .count()
            * num_challenges
    }

    /// Get `PublicSubTableValues` corresponding to `self`
    #[must_use]
    pub fn get_values<F: Field>(
        &self,
        trace: &TableKindArray<Vec<PolynomialValues<F>>>,
    ) -> PublicSubTableValues<F> {
        let trace_table = &trace[self.table.kind];
        let columns = self
            .table
            .columns
            .clone()
            .into_iter()
            .map(|col| col.map(F::from_noncanonical_i64))
            .collect_vec();
        let filter = self
            .table
            .filter_column
            .clone()
            .map(F::from_noncanonical_i64);
        let columns_if_filter_at_i = |i| -> Option<Vec<F>> {
            filter.eval_table(trace_table, i).is_one().then_some(
                columns
                    .iter()
                    .map(|column| column.eval_table(trace_table, i))
                    .collect_vec(),
            )
        };
        (0..trace_table[0].len())
            .filter_map(columns_if_filter_at_i)
            .collect_vec()
    }

    /// Create the z polynomial, and fill up the data required to prove
    /// in `CtlZdata`
    pub(crate) fn get_ctlz_data<F: Field>(
        &self,
        trace: &TableKindArray<Vec<PolynomialValues<F>>>,
        challenge: GrandProductChallenge<F>,
    ) -> CtlZData<F> {
        let z = partial_sums(
            &trace[self.table.kind],
            &self.table.columns,
            &self.table.filter_column,
            challenge,
        );
        CtlZData {
            z,
            challenge,
            columns: self.table.columns.clone(),
            filter_column: self.table.filter_column.clone(),
        }
    }

    /// Returns virtual targets corresponding to `PublicSubTableValues`
    pub fn to_targets<F: RichField + Extendable<D>, const D: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
    ) -> PublicSubTableValuesTarget {
        (0..self.num_rows)
            .map(|_| {
                (0..self.table.columns.len())
                    .map(|_| builder.add_virtual_target())
                    .collect_vec()
            })
            .collect_vec()
    }
}
pub type RowPublicValues<F> = Vec<Vec<F>>;

#[must_use]
#[allow(clippy::module_name_repetitions)]
pub fn public_sub_table_data_and_values<F: RichField, const D: usize>(
    trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    public_sub_tables: &[PublicSubTable],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> (
    TableKindArray<CtlData<F>>,
    TableKindArray<Vec<PublicSubTableValues<F>>>,
) {
    let mut public_sub_table_data_per_table = all_kind!(|_kind| CtlData::default());
    let mut public_sub_table_values_per_table = all_kind!(|_kind| Vec::default());
    for (&challenge, public_sub_table) in iproduct!(&ctl_challenges.challenges, public_sub_tables) {
        public_sub_table_data_per_table[public_sub_table.table.kind]
            .zs_columns
            .push(public_sub_table.get_ctlz_data(trace_poly_values, challenge));
    }
    for public_sub_table in public_sub_tables {
        public_sub_table_values_per_table[public_sub_table.table.kind]
            .push(public_sub_table.get_values(trace_poly_values));
    }
    (
        public_sub_table_data_per_table,
        public_sub_table_values_per_table,
    )
}

/// For each `PublicSubTableValues`, returns the compressed value
/// created according to each `challenge`
#[must_use]
pub fn reduce_public_sub_tables_values<F: Field>(
    public_sub_table_values: &TableKindArray<Vec<PublicSubTableValues<F>>>,
    challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<Vec<F>> {
    all_kind!(|kind| {
        challenges
            .challenges
            .iter()
            .flat_map(|&challenge| {
                let sub_tables = &public_sub_table_values[kind];
                sub_tables
                    .iter()
                    .map(|sub_table| {
                        sub_table
                            .iter()
                            .map(|row| challenge.combine(row).inverse())
                            .sum()
                    })
                    .collect_vec()
            })
            .collect_vec()
    })
}

pub fn reduce_public_sub_table_targets<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    challenge: &GrandProductChallenge<Target>,
    targets: &PublicSubTableValuesTarget,
) -> Target {
    let all_targets = targets
        .iter()
        .map(|row| {
            let mut combined = reduce_with_powers_circuit(builder, row, challenge.beta);
            combined = builder.add(combined, challenge.gamma);
            builder.inverse(combined)
        })
        .collect_vec();
    builder.add_many(all_targets)
}
