use std::borrow::Borrow;

use anyhow::{ensure, Result};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;
use thiserror::Error;

use crate::stark::mozak_stark::{Table, NUM_TABLES};
use crate::stark::permutation::{GrandProductChallenge, GrandProductChallengeSet};
use crate::stark::proof::StarkProof;

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("Non-binary filter at row {0}")]
    NonBinaryFilter(usize),
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
    pub(crate) columns: Vec<Column<F>>,
    pub(crate) filter_column: Column<F>,
}

pub(crate) fn verify_cross_table_lookups<F: RichField + Extendable<D>, const D: usize>(
    cross_table_lookups: &[CrossTableLookup<F>],
    ctl_zs_lasts: &[Vec<F>; NUM_TABLES],
    config: &StarkConfig,
) -> Result<()> {
    let mut ctl_zs_openings = ctl_zs_lasts.iter().map(|v| v.iter()).collect::<Vec<_>>();
    for CrossTableLookup {
        looking_tables,
        looked_table,
    } in cross_table_lookups
    {
        for _ in 0..config.num_challenges {
            let looking_zs_prod = looking_tables
                .iter()
                .map(|table| *ctl_zs_openings[table.kind as usize].next().unwrap())
                .product::<F>();
            let looked_z = *ctl_zs_openings[looked_table.kind as usize].next().unwrap();

            ensure!(
                looking_zs_prod == looked_z,
                "Cross-table lookup verification failed for {:?}->{:?} ({} != {})",
                looking_tables[0].kind,
                looked_table.kind,
                looking_zs_prod,
                looked_z
            );
        }
    }
    debug_assert!(ctl_zs_openings.iter_mut().all(|iter| iter.next().is_none()));

    Ok(())
}

pub(crate) fn cross_table_lookup_data<F: RichField, const D: usize>(
    trace_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    cross_table_lookups: &[CrossTableLookup<F>],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> [CtlData<F>; NUM_TABLES] {
    let mut ctl_data_per_table = [0; NUM_TABLES].map(|_| CtlData::default());
    for CrossTableLookup {
        looking_tables,
        looked_table,
    } in cross_table_lookups
    {
        log::debug!("Processing CTL for {:?}", looked_table.kind);
        for &challenge in &ctl_challenges.challenges {
            let zs_looking = looking_tables.iter().map(|looking_table| {
                partial_products(
                    &trace_poly_values[looking_table.kind as usize],
                    &looking_table.columns,
                    &looking_table.filter_column,
                    challenge,
                )
            });
            let z_looked = partial_products(
                &trace_poly_values[looked_table.kind as usize],
                &looked_table.columns,
                &looked_table.filter_column,
                challenge,
            );

            debug_assert_eq!(
                zs_looking
                    .clone()
                    .map(|z| *z.values.last().unwrap())
                    .product::<F>(),
                *z_looked.values.last().unwrap()
            );

            for (looking_table, z) in looking_tables.iter().zip(zs_looking) {
                ctl_data_per_table[looking_table.kind as usize]
                    .zs_columns
                    .push(CtlZData {
                        z,
                        challenge,
                        columns: looking_table.columns.clone(),
                        filter_column: looking_table.filter_column.clone(),
                    });
            }
            ctl_data_per_table[looked_table.kind as usize]
                .zs_columns
                .push(CtlZData {
                    z: z_looked,
                    challenge,
                    columns: looked_table.columns.clone(),
                    filter_column: looked_table.filter_column.clone(),
                });
        }
    }
    ctl_data_per_table
}

fn partial_products<F: Field>(
    trace: &[PolynomialValues<F>],
    columns: &[Column<F>],
    filter_column: &Column<F>,
    challenge: GrandProductChallenge<F>,
) -> PolynomialValues<F> {
    let mut partial_prod = F::ONE;
    let degree = trace[0].len();
    let mut res = Vec::with_capacity(degree);
    for i in 0..degree {
        let filter = filter_column.eval_table(trace, i);

        if filter.is_one() {
            let evals = columns
                .iter()
                .map(|c| c.eval_table(trace, i))
                .collect::<Vec<_>>();
            partial_prod *= challenge.combine(evals.iter());
        } else {
            assert_eq!(filter, F::ZERO, "Non-binary filter?");
        };
        res.push(partial_prod);
    }
    res.into()
}

/// Represent a linear combination of columns.
#[derive(Clone, Debug)]
pub struct Column<F: Field> {
    linear_combination: Vec<(usize, F)>,
    constant: F,
}

impl<F: Field> Column<F> {
    #[must_use]
    pub fn always() -> Self {
        Column {
            linear_combination: vec![],
            constant: F::ONE,
        }
    }

    #[must_use]
    pub fn single(c: usize) -> Self {
        Self {
            linear_combination: vec![(c, F::ONE)],
            constant: F::ZERO,
        }
    }

    pub fn constant(constant: F) -> Self {
        Self {
            linear_combination: vec![],
            constant,
        }
    }

    pub fn zero() -> Self { Self::constant(F::ZERO) }

    pub fn singles<I: IntoIterator<Item = impl Borrow<usize>>>(
        cs: I,
    ) -> impl Iterator<Item = Self> {
        cs.into_iter().map(|c| Self::single(*c.borrow()))
    }

    pub fn eval<FE, P, const D: usize>(&self, v: &[P]) -> P
    where
        FE: FieldExtension<D, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        self.linear_combination
            .iter()
            .map(|&(c, f)| v[c] * FE::from_basefield(f))
            .sum::<P>()
            + FE::from_basefield(self.constant)
    }

    /// Evaluate on an row of a table given in column-major form.
    pub fn eval_table(&self, table: &[PolynomialValues<F>], row: usize) -> F {
        self.linear_combination
            .iter()
            .map(|&(c, f)| table[c].values[row] * f)
            .sum::<F>()
            + self.constant
    }

    pub fn eval_circuit<const D: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        v: &[ExtensionTarget<D>],
    ) -> ExtensionTarget<D>
    where
        F: RichField + Extendable<D>, {
        let pairs = self
            .linear_combination
            .iter()
            .map(|&(c, f)| {
                (
                    v[c],
                    builder.constant_extension(F::Extension::from_basefield(f)),
                )
            })
            .collect::<Vec<_>>();
        let constant = builder.constant_extension(F::Extension::from_basefield(self.constant));
        builder.inner_product_extension(F::ONE, constant, pairs)
    }
}

#[allow(unused)]
#[derive(Clone, Debug)]
pub struct CrossTableLookup<F: Field> {
    looking_tables: Vec<Table<F>>,
    looked_table: Table<F>,
}

impl<F: Field> CrossTableLookup<F> {
    /// Instantiates a new cross table lookup between 2 tables.
    ///
    /// # Panics
    /// Panics if the two tables do not have equal number of columns.
    pub fn new(looking_tables: Vec<Table<F>>, looked_table: Table<F>) -> Self {
        assert!(looking_tables
            .iter()
            .all(|twc| twc.columns.len() == looked_table.columns.len()));
        Self {
            looking_tables,
            looked_table,
        }
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
    pub(crate) columns: &'a [Column<F>],
    pub(crate) filter_column: &'a Column<F>,
}

impl<'a, F: RichField + Extendable<D>, const D: usize>
    CtlCheckVars<'a, F, F::Extension, F::Extension, D>
{
    pub(crate) fn from_proofs<C: GenericConfig<D, F = F>>(
        proofs: &[StarkProof<F, C, D>; NUM_TABLES],
        cross_table_lookups: &'a [CrossTableLookup<F>],
        ctl_challenges: &'a GrandProductChallengeSet<F>,
        num_permutation_zs: &[usize; NUM_TABLES],
    ) -> [Vec<Self>; NUM_TABLES] {
        let mut ctl_zs = proofs
            .iter()
            .zip(num_permutation_zs)
            .map(|(p, &num_perms)| {
                let openings = &p.openings;
                let ctl_zs = openings.permutation_ctl_zs.iter().skip(num_perms);
                let ctl_zs_next = openings.permutation_ctl_zs_next.iter().skip(num_perms);
                ctl_zs.zip(ctl_zs_next)
            })
            .collect::<Vec<_>>();

        let mut ctl_vars_per_table = [0; NUM_TABLES].map(|_| vec![]);
        for CrossTableLookup {
            looking_tables,
            looked_table,
        } in cross_table_lookups
        {
            for &challenges in &ctl_challenges.challenges {
                for table in looking_tables {
                    let (looking_z, looking_z_next) = ctl_zs[table.kind as usize].next().unwrap();
                    ctl_vars_per_table[table.kind as usize].push(Self {
                        local_z: *looking_z,
                        next_z: *looking_z_next,
                        challenges,
                        columns: &table.columns,
                        filter_column: &table.filter_column,
                    });
                }

                let (looked_z, looked_z_next) = ctl_zs[looked_table.kind as usize].next().unwrap();
                ctl_vars_per_table[looked_table.kind as usize].push(Self {
                    local_z: *looked_z,
                    next_z: *looked_z_next,
                    challenges,
                    columns: &looked_table.columns,
                    filter_column: &looked_table.filter_column,
                });
            }
        }
        ctl_vars_per_table
    }
}
pub(crate) fn eval_cross_table_lookup_checks<F, FE, P, S, const D: usize, const D2: usize>(
    vars: StarkEvaluationVars<FE, P, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
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
        let combine = |v: &[P]| -> P {
            let evals = columns.iter().map(|c| c.eval(v)).collect::<Vec<_>>();
            challenges.combine(evals.iter())
        };
        let filter = |v: &[P]| -> P { filter_column.eval(v) };
        let local_filter = filter(vars.local_values);
        let next_filter = filter(vars.next_values);
        let select = |filter, x| filter * x + P::ONES - filter;

        // Check value of `Z(1)`
        consumer.constraint_first_row(*local_z - select(local_filter, combine(vars.local_values)));
        // Check `Z(gw) = combination * Z(w)`
        consumer.constraint_transition(
            *next_z - *local_z * select(next_filter, combine(vars.next_values)),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ops::Deref;

    use anyhow::Result;
    use itertools::Itertools;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::polynomial::PolynomialValues;

    use super::*;
    use crate::stark::mozak_stark::{CpuTable, Lookups, RangeCheckTable, TableKind};

    struct MultiSet<F>(HashMap<Vec<F>, Vec<(TableKind, usize)>>);

    impl<F: Field> Deref for MultiSet<F> {
        type Target = HashMap<Vec<F>, Vec<(TableKind, usize)>>;

        fn deref(&self) -> &Self::Target { &self.0 }
    }

    impl<F: Field> MultiSet<F> {
        pub fn new() -> Self { MultiSet(HashMap::new()) }

        fn process_row(
            &mut self,
            trace_poly_values: &[Vec<PolynomialValues<F>>],
            table: &Table<F>,
        ) -> Result<(), LookupError> {
            let trace = &trace_poly_values[table.kind as usize];
            for i in 0..trace[0].len() {
                let filter = table.filter_column.eval_table(trace, i);
                if filter.is_one() {
                    let row = table
                        .columns
                        .iter()
                        .map(|c| c.eval_table(trace, i))
                        .collect::<Vec<_>>();
                    self.0.entry(row).or_default().push((table.kind, i));
                } else if !filter.is_zero() {
                    return Err(LookupError::NonBinaryFilter(i));
                }
            }

            Ok(())
        }
    }

    /// Specify which column(s) to find data related to lookups.
    fn lookup_data<F: Field>(col_indices: &[usize]) -> Vec<Column<F>> {
        Column::singles(col_indices).collect_vec()
    }

    /// Specify the column index of the filter column used in lookups.
    fn lookup_filter<F: Field>(col_idx: usize) -> Column<F> { Column::single(col_idx) }

    /// A generic cross lookup table.
    struct FooBarTable<F: Field>(CrossTableLookup<F>);

    impl<F: Field> Lookups<F> for FooBarTable<F> {
        /// We use the [`CpuTable`] and the [`RangeCheckTable`] to build a
        /// [`CrossTableLookup`] here, but in principle this is meant to
        /// be used generically for tests.
        fn lookups() -> CrossTableLookup<F> {
            CrossTableLookup {
                looking_tables: vec![CpuTable::new(lookup_data(&[1]), lookup_filter(2))],
                looked_table: RangeCheckTable::new(lookup_data(&[1]), lookup_filter(0)),
            }
        }
    }

    /// Check that the provided trace and cross-table lookup are consistent.
    fn check_ctl<F: Field>(
        trace_poly_values: &[Vec<PolynomialValues<F>>],
        ctl: &CrossTableLookup<F>,
    ) -> Result<(), LookupError> {
        // Maps `m` with `(table.kind, i) in m[row]` iff the `i`-th row of the table
        // is equal to `row` and the filter is 1.
        //
        // the CTL check holds iff `looking_multiset == looked_multiset`.
        let mut looking_multiset = MultiSet::<F>::new();
        let mut looked_multiset = MultiSet::<F>::new();

        for looking_table in &ctl.looking_tables {
            looking_multiset.process_row(trace_poly_values, looking_table)?;
        }

        looked_multiset.process_row(trace_poly_values, &ctl.looked_table)?;
        let empty = &vec![];

        // Check that every row in the looking tables appears in the looked table the
        // same number of times.
        for (row, looking_locations) in &looking_multiset.0 {
            let looked_locations = looked_multiset.get(row).unwrap_or(empty);
            if looking_locations.len() != looked_locations.len() {
                println!(
                    "Row {row:?} is present {l0} times in the looking tables, but
                    {l1} times in the looked table.\n\
                    Looking locations: {looking_locations:?}.\n\
                    Looked locations: {looked_locations:?}.",
                    l0 = looking_locations.len(),
                    l1 = looked_locations.len()
                );
                return Err(LookupError::InconsistentTableRows);
            }
        }

        // Check that every row in the looked tables appears in the looking table the
        // same number of times.
        for (row, looked_locations) in &looked_multiset.0 {
            let looking_locations = looking_multiset.get(row).unwrap_or(empty);
            if looking_locations.len() != looked_locations.len() {
                println!(
                    "Row {row:?} is present {l0} times in the looking tables, but
                    {l1} times in the looked table.\n\
                    Looking locations: {looking_locations:?}.\n\
                    Looked locations: {looked_locations:?}.",
                    l0 = looking_locations.len(),
                    l1 = looked_locations.len()
                );
                return Err(LookupError::InconsistentTableRows);
            }
        }

        Ok(())
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

        pub fn build(self) -> Vec<PolynomialValues<F>> { self.trace }
    }
    /// A generic cross lookup table.
    struct NonBinaryFilterTable<F: Field>(CrossTableLookup<F>);

    impl<F: Field> Lookups<F> for NonBinaryFilterTable<F> {
        /// We use the [`CpuTable`] and the [`RangeCheckTable`] to build a
        /// [`CrossTableLookup`] here, but in principle this is meant to
        /// be used generically for tests.
        fn lookups() -> CrossTableLookup<F> {
            CrossTableLookup {
                looking_tables: vec![CpuTable::new(lookup_data(&[0]), lookup_filter(0))],
                looked_table: RangeCheckTable::new(lookup_data(&[1]), lookup_filter(0)),
            }
        }
    }

    /// Create a table with a filter column that's non-binary, which should
    /// cause our manual checks to fail.
    #[test]
    fn test_ctl_non_binary_filters() {
        type F = GoldilocksField;

        let dummy_cross_table_lookup: CrossTableLookup<F> = NonBinaryFilterTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(2).set_values(1, 5).build();
        let bar_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(0).set_values(1, 5).build();
        let traces = vec![foo_trace, bar_trace];
        assert!(matches!(
            check_ctl(&traces, &dummy_cross_table_lookup).unwrap_err(),
            LookupError::NonBinaryFilter(0)
        ));
    }

    /// Create a trace with inconsistent values, which should
    /// cause our manual checks to fail.
    /// Here, `foo_trace` has all values in column 1 set to 4,
    /// while `bar_trace` has all values in column 1 set to 5.
    /// Since [`FooBarTable`] has defined column 1 to contain lookup data,
    /// our manual checks will fail this test.
    #[test]
    fn test_ctl_inconsistent_tables() {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup<F> = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(2).set_values(1, 4).build();
        let bar_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(0).set_values(1, 5).build();
        let traces = vec![foo_trace, bar_trace];
        assert!(matches!(
            check_ctl(&traces, &dummy_cross_table_lookup).unwrap_err(),
            LookupError::InconsistentTableRows
        ));
    }

    /// Happy path test where all checks go as plan.
    #[test]
    fn test_ctl() -> Result<()> {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup<F> = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(2).set_values(1, 5).build();
        let bar_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(0).set_values(1, 5).build();
        let traces = vec![foo_trace, bar_trace];
        check_ctl(&traces, &dummy_cross_table_lookup)?;

        Ok(())
    }
}
