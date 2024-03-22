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
use crate::stark::mozak_stark::{all_kind, PublicInputs, Table, TableKindArray};
use crate::stark::permutation::challenge::GrandProductChallengeSet;

/// Specifies a table whose rows are to be made public, according to filter
/// column
#[derive(Clone, Debug)]
pub struct MakeRowsPublic {
    pub table: Table,
}
impl MakeRowsPublic {
    #[must_use]
    pub fn new(table: Table) -> Self { Self { table } }
}

pub(crate) fn open_rows_public_data<F: RichField, const D: usize>(
    trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    open_public: &[MakeRowsPublic],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<Option<CtlData<F>>> {
    let mut open_public_data_per_table = all_kind!(|_kind| None);
    for &challenge in &ctl_challenges.challenges {
        for MakeRowsPublic { table } in open_public {
            if open_public_data_per_table[table.kind].is_none() {
                open_public_data_per_table[table.kind] = Some(CtlData::default());
            }
            log::debug!("Processing Open public for {:?}", table.kind);

            let make_z = |table: &Table| {
                partial_sums(
                    &trace_poly_values[table.kind],
                    &table.columns,
                    &table.filter_column,
                    challenge,
                )
            };

            if let Some(ctl) = open_public_data_per_table[table.kind].as_mut() {
                ctl.zs_columns.push(CtlZData {
                    z: make_z(table),
                    challenge,
                    columns: table.columns.clone(),
                    filter_column: table.filter_column.clone(),
                });
            };
        }
    }
    open_public_data_per_table
}

/// For each table, Creates the sum of inverses of public data which needs to be
/// matched against final row opening of z polynomial, for the corresponding
/// instance of `MakeRowsPublic` for that table.
pub fn reduce_public_input_for_make_rows_public<F: Field>(
    _public_input: &PublicInputs<F>,
    _challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<Option<Vec<F>>> {
    all_kind!(|_kind| None)
}
