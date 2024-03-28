//! To make certain rows of columns (specified by a filter column), public, we
//! use an idea similar to what we do in CTL. We create a z polynomial for
//! every such instance which is running sum of `filter_i/combine(columns_i)`
//! where `filter_i` = 1 if we want to make the ith row `columns_i` public.
//! Now we let verifer compute the same sum, from public values to final
//! proof. Then he compares it against ! the former sum (as opening of z
//! polynomial at last row)
use itertools::{iproduct, Itertools};
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use crate::cross_table_lookup::{partial_sums, CtlData, CtlZData};
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};

/// Specifies a table whose rows are to be made public, according to filter
/// column
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct PublicSubTable {
    pub table: Table,
    pub num_rows: usize,
}
#[allow(clippy::module_name_repetitions)]
pub type PublicSubTableValues<F> = Vec<Vec<F>>;
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

    #[must_use]
    pub fn get_values<F: Field>(
        &self,
        trace: &TableKindArray<Vec<PolynomialValues<F>>>,
    ) -> PublicSubTableValues<F> {
        let trace_table = &trace[self.table.kind];
        let columns_if_filter_at_i = |i| -> Option<Vec<F>> {
            self.table
                .filter_column
                .eval_table(trace_table, i)
                .is_one()
                .then_some(
                    self.table
                        .columns
                        .iter()
                        .map(|column| column.eval_table(trace_table, i))
                        .collect_vec(),
                )
        };
        (0..trace_table[0].len())
            .filter_map(columns_if_filter_at_i)
            .collect_vec()
    }

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
    let mut open_public_data_per_table = all_kind!(|_kind| CtlData::default());
    let mut public_sub_values_data_per_table = all_kind!(|_kind| Vec::default());
    for (public_sub_table, &challenge) in iproduct!(public_sub_tables, &ctl_challenges.challenges) {
        open_public_data_per_table[public_sub_table.table.kind]
            .zs_columns
            .push(public_sub_table.get_ctlz_data(trace_poly_values, challenge));
        public_sub_values_data_per_table[public_sub_table.table.kind]
            .push(public_sub_table.get_values(trace_poly_values));
    }
    (open_public_data_per_table, public_sub_values_data_per_table)
}

/// For each table, Creates the sum of inverses of public data which needs to be
/// matched against final row opening of z polynomial, for the corresponding
/// instance of `MakeRowsPublic` for that table.
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
