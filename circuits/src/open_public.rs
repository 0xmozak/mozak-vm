use anyhow::{ensure, Result};
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;

use crate::cross_table_lookup::{partial_sums, CtlData, CtlZData};
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray};
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
) -> TableKindArray<CtlData<F>> {
    let mut open_public_data_per_table = all_kind!(|_kind| CtlData::default());
    for &challenge in &ctl_challenges.challenges {
        for OpenPublic { table } in open_public {
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
                    is_open_public: true,
                });
        }
    }
    open_public_data_per_table
}

pub(crate) fn verify_open_public<F: RichField + Extendable<D>, const D: usize>(
    open_public: &[OpenPublic],
    open_public_zs_lasts: &TableKindArray<Option<Vec<F>>>,
    reduced_public_inputs: &TableKindArray<Option<Vec<F>>>,
    config: &StarkConfig,
) -> Result<()> {
    for i in 0..config.num_challenges {
        for OpenPublic { table } in open_public {
            ensure!(
                reduced_public_inputs[table.kind].as_ref().unwrap()[i]
                    == open_public_zs_lasts[table.kind].as_ref().unwrap()[i],
                "Open public verification failed for {:?} ",
                table.kind,
            );
        }
    }

    Ok(())
}

pub fn reduce_public_input<F: Field>(
    kind: TableKind,
    public_input: &[F],
    challenges: &GrandProductChallengeSet<F>,
) -> Option<Vec<F>> {
    // match kind {
    //     TableKind::MozakMemoryInit => {
    //         let mut reduced = vec![];
    //         for challenge in challenges.challenges.iter() {
    //             reduced.push(
    //                 (0..32)
    //                     .map(|i| {
    //                         challenge
    //                             .combine(&vec![public_input[2 * i],
    // public_input[2 * i + 1]])                             .inverse()
    //                     })
    //                     .sum(),
    //             )
    //         }
    //         Some(reduced)
    //     }
    //     _ => None,
    // }
    None
}
