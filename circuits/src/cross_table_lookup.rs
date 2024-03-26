use anyhow::{ensure, Result};
use itertools::{chain, iproduct, izip, zip_eq};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::Stark;
use thiserror::Error;

pub use crate::linear_combination::Column;
use crate::linear_combination::ColumnSparse;
pub use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray, TableWithTypedOutput};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};
use crate::stark::proof::{StarkProof, StarkProofTarget};

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("Inconsistency found between looking and looked tables")]
    InconsistentTableRows,
}

#[derive(Clone, Default)]
pub struct CtlData<F: Field> {
    pub(crate) zs_columns: Vec<CtlZData<F>>,
}

impl<F: Field> CtlData<F> {
    #[must_use]
    pub fn len(&self) -> usize { self.zs_columns.len() }

    #[must_use]
    pub fn is_empty(&self) -> bool { self.zs_columns.is_empty() }

    #[must_use]
    pub fn z_polys(&self) -> Vec<PolynomialValues<F>> {
        self.zs_columns
            .iter()
            .map(|zs_column| zs_column.z.clone())
            .collect()
    }
}

/// Cross-table lookup data associated with one Z(x) polynomial.
#[derive(Clone)]
pub(crate) struct CtlZData<F: Field> {
    pub(crate) z: PolynomialValues<F>,
    pub(crate) challenge: GrandProductChallenge<F>,
    pub(crate) columns: Vec<Column>,
    pub(crate) filter_column: Column,
}

pub(crate) fn verify_cross_table_lookups<F: RichField + Extendable<D>, const D: usize>(
    cross_table_lookups: &[CrossTableLookup],
    ctl_zs_lasts: &TableKindArray<Vec<F>>,
    config: &StarkConfig,
) -> Result<()> {
    let mut ctl_zs_openings = ctl_zs_lasts.each_ref().map(|v| v.iter());
    for _ in 0..config.num_challenges {
        for CrossTableLookup {
            looking_tables,
            looked_tables,
        } in cross_table_lookups
        {
            let looking_zs_sum = looking_tables
                .iter()
                .map(|table| *ctl_zs_openings[table.kind].next().unwrap())
                .sum::<F>();
            let looked_zs_sum = looked_tables
                .iter()
                .map(|table| *ctl_zs_openings[table.kind].next().unwrap())
                .sum::<F>();

            ensure!(
                looking_zs_sum == looked_zs_sum,
                "Cross-table lookup verification failed for {:?}->{:?} ({} != {})",
                looking_tables.iter().map(|table| table.kind),
                looked_tables.iter().map(|table| table.kind),
                looking_zs_sum,
                looked_zs_sum,
            );
        }
    }
    debug_assert!(ctl_zs_openings.iter_mut().all(|iter| iter.next().is_none()));

    Ok(())
}

pub(crate) fn cross_table_lookup_data<F: RichField, const D: usize>(
    trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    cross_table_lookups: &[CrossTableLookup],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> TableKindArray<CtlData<F>> {
    let mut ctl_data_per_table = all_kind!(|_kind| CtlData::default());
    for &challenge in &ctl_challenges.challenges {
        for CrossTableLookup {
            looking_tables,
            looked_tables,
        } in cross_table_lookups
        {
            log::debug!(
                "Processing CTL for {:?}",
                looked_tables
                    .iter()
                    .map(|table| table.kind)
                    .collect::<Vec<_>>()
            );

            let make_z = |table: &Table| {
                partial_sums(
                    &trace_poly_values[table.kind],
                    &table.columns,
                    &table.filter_column,
                    challenge,
                )
            };
            let zs_looking = looking_tables.iter().map(make_z);
            let zs_looked = looked_tables.iter().map(make_z);

            debug_assert_eq!(
                zs_looking
                    .clone()
                    .map(|z| *z.values.last().unwrap())
                    .sum::<F>(),
                zs_looked
                    .clone()
                    .map(|z| *z.values.last().unwrap())
                    .sum::<F>(),
            );

            for (table, z) in chain!(
                izip!(looking_tables, zs_looking),
                izip!(looked_tables, zs_looked)
            ) {
                ctl_data_per_table[table.kind].zs_columns.push(CtlZData {
                    z,
                    challenge,
                    columns: table.columns.clone(),
                    filter_column: table.filter_column.clone(),
                });
            }
        }
    }
    ctl_data_per_table
}

// pub fn prep_columns<F: Field>(columns: &[Column], challenge:
// GrandProductChallenge<F>) -> ColumnSparse<F> {

//     // let beta = F::from_noncanonical_i64(n)
//     // reduce_with_powers(terms, FE::from_basefield(self.beta))
//     // + FE::from_basefield(self.gamma)
//     todo!()
// }

fn partial_sums<F: Field>(
    trace: &[PolynomialValues<F>],
    columns: &[Column],
    filter_column: &Column,
    challenge: GrandProductChallenge<F>,
) -> PolynomialValues<F> {
    // design of table looks like  this
    //       |  filter  |   value   |  partial_sum                       |
    //       |    1     |    x_1    |  1/combine(x_3)                    |
    //       |    0     |    x_2    |  1/combine(x_3)  + 1/combine(x_1)  |
    //       |    1     |    x_3    |  1/combine(x_1)  + 1/combine(x_1)  |
    // (where combine(vals) = gamma + reduced_sum(vals))
    // this is done so that now transition constraint looks like
    //       z_next = z_local + filter_local/combine_local
    // That is, there is no need for reconstruction of value_next.
    // In current design which uses lv and nv values from columns to construct the
    // final value_local, its impossible to construct value_next from lv and nv
    // values of current row

    let get_multiplicity = |&i| -> F { filter_column.eval_table(trace, i) };

    let get_combined = |&i| -> F {
        let evals = columns
            .iter()
            .map(|c| c.eval_table(trace, i))
            .collect::<Vec<_>>();
        challenge.combine(evals.iter())
        // reduce_with_powers(terms, FE::from_basefield(self.beta))
        // + FE::from_basefield(self.gamma)
    };

    let degree = trace[0].len();
    let mut degrees = (0..degree).collect::<Vec<_>>();
    degrees.rotate_right(1);

    let multiplicities: Vec<F> = degrees.iter().map(get_multiplicity).collect();
    let data: Vec<F> = degrees.iter().map(get_combined).collect();
    let inv_data = F::batch_multiplicative_inverse(&data);

    izip!(multiplicities, inv_data)
        .scan(F::ZERO, |partial_sum: &mut F, (multiplicity, inv)| {
            *partial_sum += multiplicity * inv;
            Some(*partial_sum)
        })
        .collect::<Vec<_>>()
        .into()
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct CrossTableLookupWithTypedOutput<Row> {
    pub looking_tables: Vec<TableWithTypedOutput<Row>>,
    pub looked_tables: Vec<TableWithTypedOutput<Row>>,
}

// This is a little trick, so that we can use `CrossTableLookup` as a
// constructor, but only when the type parameter Row = Vec<Column>.
// TODO(Matthias): See if we can do the same trick for `table_impl`.
#[allow(clippy::module_name_repetitions)]
pub type CrossTableLookupUntyped = CrossTableLookupWithTypedOutput<Vec<Column>>;
pub use CrossTableLookupUntyped as CrossTableLookup;

impl<Row: IntoIterator<Item = Column>> CrossTableLookupWithTypedOutput<Row> {
    pub fn to_untyped_output(self) -> CrossTableLookup {
        let looked_tables = self
            .looked_tables
            .into_iter()
            .map(TableWithTypedOutput::to_untyped_output)
            .collect();
        let looking_tables = self
            .looking_tables
            .into_iter()
            .map(TableWithTypedOutput::to_untyped_output)
            .collect();
        CrossTableLookup {
            looking_tables,
            looked_tables,
        }
    }
}

impl<Row> CrossTableLookupWithTypedOutput<Row> {
    /// Instantiates a new cross table lookup between 2 tables.
    ///
    /// # Panics
    /// Panics if the two tables do not have equal number of columns.
    #[must_use]
    pub fn new(
        looking_tables: Vec<TableWithTypedOutput<Row>>,
        looked_tables: Vec<TableWithTypedOutput<Row>>,
    ) -> Self {
        Self {
            looking_tables,
            looked_tables,
        }
    }

    #[must_use]
    pub fn num_ctl_zs(ctls: &[Self], table: TableKind, num_challenges: usize) -> usize {
        ctls.iter()
            .flat_map(|ctl| chain!(&ctl.looked_tables, &ctl.looking_tables))
            .filter(|twc| twc.kind == table)
            .count()
            * num_challenges
    }
}

#[derive(Clone)]
pub struct CtlCheckVars<'a, F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) local_z: P,
    pub(crate) next_z: P,
    pub(crate) challenges: GrandProductChallenge<F>,
    pub(crate) columns: &'a [Column],
    pub(crate) filter_column: &'a Column,
}

impl<'a, F: RichField + Extendable<D>, const D: usize>
    CtlCheckVars<'a, F, F::Extension, F::Extension, D>
{
    pub(crate) fn from_proofs<C: GenericConfig<D, F = F>>(
        proofs: &TableKindArray<StarkProof<F, C, D>>,
        cross_table_lookups: &'a [CrossTableLookup],
        ctl_challenges: &'a GrandProductChallengeSet<F>,
    ) -> TableKindArray<Vec<Self>> {
        let mut ctl_zs = proofs
            .each_ref()
            .map(|p| izip!(&p.openings.ctl_zs, &p.openings.ctl_zs_next));

        let mut ctl_vars_per_table = all_kind!(|_kind| vec![]);
        let ctl_chain = cross_table_lookups.iter().flat_map(
            |CrossTableLookup {
                 looking_tables,
                 looked_tables,
             }| chain!(looking_tables, looked_tables),
        );
        for (&challenges, table) in iproduct!(&ctl_challenges.challenges, ctl_chain) {
            let (&local_z, &next_z) = ctl_zs[table.kind].next().unwrap();
            ctl_vars_per_table[table.kind].push(Self {
                local_z,
                next_z,
                challenges,
                columns: &table.columns,
                filter_column: &table.filter_column,
            });
        }
        ctl_vars_per_table
    }
}

pub(crate) fn eval_cross_table_lookup_checks<F, FE, P, S, const D: usize, const D2: usize>(
    vars: &S::EvaluationFrame<FE, P, D2>,
    ctl_vars: &[CtlCheckVars<F, FE, P, D2>],
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>, {
    for lookup_vars in ctl_vars {
        let CtlCheckVars {
            local_z,
            next_z,
            challenges,
            columns,
            filter_column,
        } = lookup_vars;
        let local_values = vars.get_local_values();
        let next_values = vars.get_next_values();

        let combine = |lv: &[P], nv: &[P]| -> P {
            let evals = columns.iter().map(|c| c.eval(lv, nv)).collect::<Vec<_>>();
            challenges.combine(evals.iter())
        };
        let combination = combine(local_values, next_values);
        let multiplicity = |lv: &[P], nv: &[P]| -> P { filter_column.eval(lv, nv) };
        let multiplicity = multiplicity(local_values, next_values);

        // Check value of `Z(1) = filter(w^(n-1))/combined(w^(n-1))`
        consumer.constraint_last_row(*next_z * combination - multiplicity);

        // Check `Z(gw) - Z(w) = filter(w)/combined(w)`
        consumer.constraint_transition((*next_z - *local_z) * combination - multiplicity);
    }
}

#[derive(Clone)]
pub struct CtlCheckVarsTarget<'a, const D: usize> {
    pub local_z: ExtensionTarget<D>,
    pub next_z: ExtensionTarget<D>,
    pub challenges: GrandProductChallenge<Target>,
    pub columns: &'a [Column],
    pub filter_column: &'a Column,
}

impl<'a, const D: usize> CtlCheckVarsTarget<'a, D> {
    #[must_use]
    pub fn from_proof(
        table: TableKind,
        proof: &StarkProofTarget<D>,
        cross_table_lookups: &'a [CrossTableLookup],
        ctl_challenges: &'a GrandProductChallengeSet<Target>,
    ) -> Vec<Self> {
        let ctl_zs = izip!(&proof.openings.ctl_zs, &proof.openings.ctl_zs_next);

        let ctl_chain = cross_table_lookups.iter().flat_map(
            |CrossTableLookup {
                 looking_tables,
                 looked_tables,
             }| chain!(looking_tables, looked_tables).filter(|twc| twc.kind == table),
        );
        zip_eq(ctl_zs, iproduct!(&ctl_challenges.challenges, ctl_chain))
            .map(|((&local_z, &next_z), (&challenges, table))| Self {
                local_z,
                next_z,
                challenges,
                columns: &table.columns,
                filter_column: &table.filter_column,
            })
            .collect()
    }
}

pub fn eval_cross_table_lookup_checks_circuit<
    S: Stark<F, D>,
    F: RichField + Extendable<D>,
    const D: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    vars: &S::EvaluationFrameTarget,
    ctl_vars: &[CtlCheckVarsTarget<D>],
    consumer: &mut RecursiveConstraintConsumer<F, D>,
) {
    for lookup_vars in ctl_vars {
        let CtlCheckVarsTarget {
            local_z,
            next_z,
            challenges,
            columns,
            filter_column,
        }: &CtlCheckVarsTarget<D> = lookup_vars;

        let local_values = vars.get_local_values();
        let next_values = vars.get_next_values();

        let evals: Vec<_> = columns
            .iter()
            .map(|c| c.eval_circuit(builder, local_values, next_values))
            .collect();
        let combined = challenges.combine_circuit(builder, &evals);

        let multiplicity = filter_column.eval_circuit(builder, local_values, next_values);

        // Check value of `Z(1) = filter(w^(n-1))/combined(w^(n-1))`
        let last_row = builder.mul_sub_extension(*next_z, combined, multiplicity);
        consumer.constraint_last_row(builder, last_row);

        // Check `Z(gw) - Z(w) = filter(w)/combined(w)`
        let diff = builder.sub_extension(*next_z, *local_z);
        let transition = builder.mul_sub_extension(diff, combined, multiplicity);
        consumer.constraint_transition(builder, transition);
    }
}

pub mod ctl_utils {
    use std::collections::HashMap;

    use anyhow::Result;
    use derive_more::{Deref, DerefMut};
    use plonky2::field::extension::Extendable;
    use plonky2::field::polynomial::PolynomialValues;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::RichField;

    use crate::cross_table_lookup::{CrossTableLookup, LookupError};
    use crate::stark::mozak_stark::{MozakStark, Table, TableKind, TableKindArray};

    #[derive(Clone, Debug, Default, Deref, DerefMut)]
    struct MultiSet<F>(HashMap<Vec<F>, Vec<(TableKind, F)>>);

    impl<F: Field> MultiSet<F> {
        fn process_row(
            &mut self,
            trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
            table: &Table,
        ) {
            let trace = &trace_poly_values[table.kind];
            for i in 0..trace[0].len() {
                let filter = table.filter_column.eval_table(trace, i);
                if filter.is_nonzero() {
                    let row = table
                        .columns
                        .iter()
                        .map(|c| c.eval_table(trace, i))
                        .collect::<Vec<_>>();
                    self.entry(row).or_default().push((table.kind, filter));
                };
            }
        }
    }

    pub fn check_single_ctl<F: Field>(
        trace_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
        // TODO(Matthias): make this one work with CrossTableLookupNamed, instead of having to
        // forget the types first.  That should also help with adding better debug messages.
        ctl: &CrossTableLookup,
    ) -> Result<(), LookupError> {
        /// Sums and compares the multiplicities of the given looking and looked
        /// locations previously processed.
        ///
        /// The CTL check holds iff `looking_multiplicity ==
        /// looked_multiplicity`.
        fn check_multiplicities<F: Field>(
            row: &[F],
            looking_locations: &[(TableKind, F)],
            looked_locations: &[(TableKind, F)],
        ) -> Result<(), LookupError> {
            let looking_multiplicity = looking_locations.iter().map(|l| l.1).sum::<F>();
            let looked_multiplicity = looked_locations.iter().map(|l| l.1).sum::<F>();
            if looking_multiplicity != looked_multiplicity {
                println!(
                    "Row {row:?} has multiplicity {looking_multiplicity} in the looking tables, but
                    {looked_multiplicity} in the looked table.\n\
                    Looking locations: {looking_locations:?}.\n\
                    Looked locations: {looked_locations:?}.",
                );
                return Err(LookupError::InconsistentTableRows);
            }

            Ok(())
        }

        // Maps `m` with `(table.kind, multiplicity) in m[row]`
        let mut looking_multiset = MultiSet::<F>::default();
        let mut looked_multiset = MultiSet::<F>::default();

        for looking_table in &ctl.looking_tables {
            looking_multiset.process_row(trace_poly_values, looking_table);
        }

        for looked_table in &ctl.looked_tables {
            looked_multiset.process_row(trace_poly_values, looked_table);
        }

        let empty = &vec![];
        // Check that every row in the looking tables appears in the looked table the
        // same number of times.
        for (row, looking_locations) in &looking_multiset.0 {
            let looked_locations = looked_multiset.get(row).unwrap_or(empty);
            check_multiplicities(row, looking_locations, looked_locations)?;
        }

        // Check that every row in the looked tables appears in the looking table the
        // same number of times.
        for (row, looked_locations) in &looked_multiset.0 {
            let looking_locations = looking_multiset.get(row).unwrap_or(empty);
            check_multiplicities(row, looking_locations, looked_locations)?;
        }

        Ok(())
    }
    pub fn debug_ctl<F: RichField + Extendable<D>, const D: usize>(
        traces_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
        mozak_stark: &MozakStark<F, D>,
    ) {
        mozak_stark
            .cross_table_lookups
            .iter()
            .for_each(|ctl| check_single_ctl(traces_poly_values, ctl).unwrap());
    }
}

// TODO(Matthias): restore the tests from before https://github.com/0xmozak/mozak-vm/pull/1371
