use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;

use crate::cross_table_lookup::{partial_sums, CtlData, CtlZData};
use crate::stark::mozak_stark::{all_kind, PublicInputs, Table, TableKindArray};
use crate::stark::permutation::challenge::GrandProductChallengeSet;

#[derive(Clone, Debug)]
pub struct OpenPublic {
    pub table: Table,
}
impl OpenPublic {
    pub fn new(table: Table) -> Self { Self { table } }
}

pub(crate) fn open_public_data<F: RichField, const D: usize>(
    trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    open_public: &[OpenPublic],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<Option<CtlData<F>>> {
    let mut open_public_data_per_table = all_kind!(|_kind| None);
    for &challenge in &ctl_challenges.challenges {
        for OpenPublic { table } in open_public {
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

            open_public_data_per_table[table.kind].as_mut().map(|ctl| {
                ctl.zs_columns.push(CtlZData {
                    z: make_z(table),
                    challenge,
                    columns: table.columns.clone(),
                    filter_column: table.filter_column.clone(),
                });
            });
        }
    }
    open_public_data_per_table
}

pub fn reduce_public_input<F: Field>(
    _public_input: &PublicInputs<F>,
    _challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<Option<Vec<F>>> {
    all_kind!(|kind| {
        match kind {
            _ => None,
        }
    })
}
