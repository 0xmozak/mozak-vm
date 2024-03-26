use itertools::{chain, Itertools};
/// ! To make certain rows of columns (specified by a filter column), public, we
/// use an idea similar to what we do in CTL ! We create a z polynomial for
/// every such instance which is running sum of `filter_i/combine(columns_i)`
/// ! where `filter_i` = 1 if we want to make the ith row `columns_i` public.
/// ! Now we let verifer compute the same sum, from public values to final
/// proof. Then he compares it against ! the former sum (as opening of z
/// polynomial at last row)
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;

use crate::cross_table_lookup::{partial_sums, CtlData, CtlZData};
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray};
use crate::stark::permutation::challenge::GrandProductChallengeSet;

/// Specifies a table whose rows are to be made public, according to filter
/// column
#[derive(Clone, Debug)]
pub struct MakeRowsPublic(pub Table);

impl MakeRowsPublic {
    pub fn num_zs(ctls: &[Self], table: TableKind, num_challenges: usize) -> usize {
        ctls.iter()
            .map(|Self(table)| table)
            .filter(|twc| twc.kind == table)
            .count()
            * num_challenges
    }
}
pub type RowPublicValues<F> = Vec<Vec<F>>;

pub(crate) fn open_rows_public_data<F: RichField, const D: usize>(
    trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    open_public: &[MakeRowsPublic],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<CtlData<F>> {
    let mut open_public_data_per_table = all_kind!(|_kind| CtlData::default());
    for &challenge in &ctl_challenges.challenges {
        for MakeRowsPublic(table) in open_public {
            log::debug!("Processing Open public for {:?}", table.kind);

            let make_z = |table: &Table| {
                partial_sums(
                    &trace_poly_values[table.kind],
                    &table.columns,
                    &table.filter_column,
                    challenge,
                )
            };

            open_public_data_per_table[table.kind]
                .zs_columns
                .push(CtlZData {
                    z: make_z(table),
                    challenge,
                    columns: table.columns.clone(),
                    filter_column: table.filter_column.clone(),
                });
        }
    }
    open_public_data_per_table
}

/// For each table, Creates the sum of inverses of public data which needs to be
/// matched against final row opening of z polynomial, for the corresponding
/// instance of `MakeRowsPublic` for that table.
pub fn reduce_public_input_for_make_rows_public<F: Field>(
    row_public_values: &TableKindArray<RowPublicValues<F>>,
    challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<Vec<F>> {
    all_kind!(|kind| challenges
        .challenges
        .iter()
        .map(|&challenge| row_public_values[kind]
            .iter()
            .map(|row| challenge.combine(row).inverse())
            .sum())
        .collect_vec())
}

pub fn get_public_row_values<F: Field>(
    trace: &TableKindArray<Vec<PolynomialValues<F>>>,
    make_row_public: &[MakeRowsPublic],
) -> TableKindArray<RowPublicValues<F>> {
    let mut public_row_values_per_table = all_kind!(|_kind| Vec::default());
    for MakeRowsPublic(table) in make_row_public {
        let trace_table = &trace[table.kind];
        let columns_if_filter_at_i = |i| -> Option<Vec<F>> {
            if table.filter_column.eval_table(&trace_table, i).is_one() {
                Some(
                    table
                        .columns
                        .iter()
                        .map(|column| column.eval_table(&trace_table, i))
                        .collect_vec(),
                )
            } else {
                None
            }
        };
        let column_values = (0..trace_table[0].len())
            .filter_map(columns_if_filter_at_i)
            .collect_vec();
        public_row_values_per_table[table.kind] = column_values;
    }
    public_row_values_per_table
}
