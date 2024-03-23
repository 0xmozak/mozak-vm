use anyhow::{ensure, Result};
use itertools::{chain, iproduct, izip, zip_eq, Itertools};
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
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray};
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
    pub fn is_empty(&self) -> bool { self.zs_columns.len() == 0 }

    #[must_use]
    pub fn z_polys(&self) -> Vec<PolynomialValues<F>> {
        self.zs_columns
            .iter()
            .map(|zs_columns| zs_columns.z.clone())
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
            looked_table,
        } in cross_table_lookups
        {
            let looking_zs_sum = looking_tables
                .iter()
                .map(|table| *ctl_zs_openings[table.kind].next().unwrap())
                .sum::<F>();
            let looked_z = *ctl_zs_openings[looked_table.kind].next().unwrap();

            ensure!(
                looking_zs_sum == looked_z,
                "Cross-table lookup verification failed for {:?}->{:?} ({} != {})",
                looking_tables[0].kind,
                looked_table.kind,
                looking_zs_sum,
                looked_z
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
            looked_table,
        } in cross_table_lookups
        {
            log::debug!("Processing CTL for {:?}", looked_table.kind);

            let make_z = |table: &Table| {
                partial_sums(
                    &trace_poly_values[table.kind],
                    &table.columns,
                    &table.filter_column,
                    challenge,
                )
            };
            let zs_looking = looking_tables.iter().map(make_z);
            let z_looked = make_z(looked_table);

            debug_assert_eq!(
                zs_looking
                    .clone()
                    .map(|z| *z.values.last().unwrap())
                    .sum::<F>(),
                *z_looked.values.last().unwrap()
            );

            for (table, z) in chain!(izip!(looking_tables, zs_looking), [(
                looked_table,
                z_looked
            )]) {
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

    // TODO(Kapil): inverse for all rows is expensive. Use batched division idea.

    let combine_and_inv_if_filter_at_i = |i| -> F {
        let multiplicity = filter_column.eval_table(trace, i);
        let evals = columns
            .iter()
            .map(|c| c.eval_table(trace, i))
            .collect::<Vec<_>>();
        multiplicity * challenge.combine(evals.iter()).inverse()
    };

    let degree = trace[0].len();
    let mut degrees = (0..degree).collect::<Vec<_>>();
    degrees.rotate_right(1);
    degrees
        .into_iter()
        .map(combine_and_inv_if_filter_at_i)
        .scan(F::ZERO, |partial_sum: &mut F, combined| {
            *partial_sum += combined;
            Some(*partial_sum)
        })
        .collect_vec()
        .into()
}

#[allow(unused)]
#[derive(Clone, Debug)]
pub struct CrossTableLookup {
    pub looking_tables: Vec<Table>,
    pub looked_table: Table,
}

impl CrossTableLookup {
    /// Instantiates a new cross table lookup between 2 tables.
    ///
    /// # Panics
    /// Panics if the two tables do not have equal number of columns.
    #[must_use]
    pub fn new(looking_tables: Vec<Table>, looked_table: Table) -> Self {
        Self {
            looking_tables,
            looked_table,
        }
    }

    #[must_use]
    pub fn num_ctl_zs(ctls: &[Self], table: TableKind, num_challenges: usize) -> usize {
        ctls.iter()
            .flat_map(|ctl| chain!([&ctl.looked_table], &ctl.looking_tables))
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
                 looked_table,
             }| chain!(looking_tables, [looked_table]),
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
                 looked_table,
             }| chain!(looking_tables, [looked_table]).filter(|twc| twc.kind == table),
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

        looked_multiset.process_row(trace_poly_values, &ctl.looked_table);

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

#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;

    use super::ctl_utils::check_single_ctl;
    use super::*;
    use crate::stark::mozak_stark::{CpuTable, Lookups, RangeCheckTable, TableKindSetBuilder};

    /// Specify which column(s) to find data related to lookups.
    /// If the lengths of `lv_col_indices` and `nv_col_indices` are not same,
    /// then we resize smaller one with empty column and then add componentwise
    fn lookup_data(lv_col_indices: &[usize], nv_col_indices: &[usize]) -> Vec<Column> {
        // use usual lv values of the rows
        let lv_columns = Column::singles(lv_col_indices);
        // use nv values of the rows
        let nv_columns = Column::singles_next(nv_col_indices);

        lv_columns
            .into_iter()
            .zip_longest(nv_columns)
            .map(|item| item.reduce(std::ops::Add::add))
            .collect()
    }

    /// Specify the column index of the filter column used in lookups.
    fn lookup_filter(col_idx: usize) -> Column { Column::single(col_idx) }

    /// A generic cross lookup table.
    struct FooBarTable;

    impl Lookups for FooBarTable {
        /// We use the [`CpuTable`] and the [`RangeCheckTable`] to build a
        /// [`CrossTableLookup`] here, but in principle this is meant to
        /// be used generically for tests.
        fn lookups() -> CrossTableLookup {
            CrossTableLookup {
                looking_tables: vec![CpuTable::new(lookup_data(&[1], &[2]), lookup_filter(0))],
                looked_table: RangeCheckTable::new(lookup_data(&[1], &[]), lookup_filter(0)),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Trace<F: Field> {
        trace: Vec<PolynomialValues<F>>,
    }

    #[derive(Default)]
    pub struct TraceBuilder<F: Field> {
        trace: Vec<PolynomialValues<F>>,
    }

    impl<F: Field> TraceBuilder<F> {
        /// Creates a new trace with the given `num_cols` and `num_rows`.
        pub fn new(num_cols: usize, num_rows: usize) -> TraceBuilder<F> {
            let mut trace = vec![];
            for _ in 0..num_cols {
                let mut values = Vec::with_capacity(num_rows);
                for _ in 0..num_rows {
                    values.push(F::rand());
                }
                trace.push(PolynomialValues::from(values));
            }

            TraceBuilder { trace }
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// zeroes.
        #[allow(unused)]
        pub fn zero(mut self, idx: usize) -> TraceBuilder<F> {
            self.trace[idx] = PolynomialValues::zero(self.trace[idx].len());

            self
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// `F::ONE`.
        pub fn one(mut self, col_idx: usize) -> TraceBuilder<F> {
            let len = self.trace[col_idx].len();
            let ones = PolynomialValues::constant(F::ONE, len);
            self.trace[col_idx] = ones;

            self
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// `value`. This is convenient for testing cross table lookups.
        pub fn set_values(mut self, col_idx: usize, value: usize) -> TraceBuilder<F> {
            let len = self.trace[col_idx].len();
            let new_v: Vec<F> = (0..len).map(|_| F::from_canonical_usize(value)).collect();
            let values = PolynomialValues::from(new_v);
            self.trace[col_idx] = values;

            self
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// alternate between `value_1` and `value_2`. Useful for testing
        /// combination of lv and nv values
        pub fn set_values_alternate(
            mut self,
            col_idx: usize,
            value_1: usize,
            value_2: usize,
        ) -> TraceBuilder<F> {
            let len = self.trace[col_idx].len();
            self.trace[col_idx] = PolynomialValues::from(
                [value_1, value_2]
                    .into_iter()
                    .cycle()
                    .take(len)
                    .map(F::from_canonical_usize)
                    .collect_vec(),
            );

            self
        }

        pub fn build(self) -> Vec<PolynomialValues<F>> { self.trace }
    }

    /// Create a trace with inconsistent values, which should
    /// cause our manual checks to fail.
    /// Here, `foo_trace` has all values in column 1 and 2 set to alternate
    /// between 2 and 3 while `bar_trace` has all values in column 1 set to
    /// 6. Since lookup data is sum of lv values of column 1 and nv values
    /// of column 2 from `foo_trace`, our manual checks will fail this test.
    #[test]
    fn test_ctl_inconsistent_tables() {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values_alternate(1, 2, 3)
            .set_values_alternate(2, 2, 3)
            .build();
        let bar_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values(1, 6)
            .build();
        let traces = TableKindSetBuilder {
            cpu_stark: foo_trace,
            rangecheck_stark: bar_trace,
            ..Default::default()
        }
        .build();
        assert!(matches!(
            check_single_ctl(&traces, &dummy_cross_table_lookup).unwrap_err(),
            LookupError::InconsistentTableRows
        ));
    }

    /// Happy path test where all checks go as plan.
    /// Here, `foo_trace` has all values in column 1 set to alternate between 2
    /// and 3, and values in column 2 set to alternate between 3 and 2 while
    /// `bar_trace` has all values in column 1 set to 5. Since lookup data
    /// is sum of lv values of column 1 and nv values of column 2 from
    /// `foo_trace`, our manual checks will pass the test
    #[test]
    fn test_ctl() -> Result<()> {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values_alternate(1, 2, 3)
            .set_values_alternate(2, 2, 3)
            .build();
        let bar_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values(1, 5)
            .build();
        let traces = TableKindSetBuilder {
            cpu_stark: foo_trace,
            rangecheck_stark: bar_trace,
            ..Default::default()
        }
        .build();
        check_single_ctl(&traces, &dummy_cross_table_lookup)?;
        Ok(())
    }
}
