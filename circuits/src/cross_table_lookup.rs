use std::borrow::Borrow;

use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use thiserror::Error;

use crate::{cpu, rangecheck};

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("Non-binary filter at row {0}")]
    NonBinaryFilter(usize),
    #[error("Inconsistency found between looking and looked tables")]
    InconsistentTableRows,
}

/// Represent a linear combination of columns.
#[derive(Clone, Debug)]
pub struct Column<F: Field> {
    linear_combination: Vec<(usize, F)>,
    constant: F,
}

impl<F: Field> Column<F> {
    #[must_use]
    pub fn single(c: usize) -> Self {
        Self {
            linear_combination: vec![(c, F::ONE)],
            constant: F::ZERO,
        }
    }

    pub fn singles<I: IntoIterator<Item = impl Borrow<usize>>>(
        cs: I,
    ) -> impl Iterator<Item = Self> {
        cs.into_iter().map(|c| Self::single(*c.borrow()))
    }

    /// Evaluate on an row of a table given in column-major form.
    pub fn eval_table(&self, table: &[PolynomialValues<F>], row: usize) -> F {
        self.linear_combination
            .iter()
            .map(|&(c, f)| table[c].values[row] * f)
            .sum::<F>()
            + self.constant
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub struct Table<F: Field> {
    kind: TableKind,
    columns: Vec<Column<F>>,
    pub(crate) filter_column: Option<Column<F>>,
}

impl<F: Field> Table<F> {
    pub fn new(kind: TableKind, columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Self {
        Self {
            kind,
            columns,
            filter_column,
        }
    }
}

/// Represents a range check table in the Mozak VM.
pub struct RangeCheckTable<F: Field>(Table<F>);

/// Represents a cpu table in the Mozak VM.
pub struct CpuTable<F: Field>(Table<F>);

impl<F: Field> RangeCheckTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Table<F> {
        Table::new(TableKind::RangeCheck, columns, filter_column)
    }
}

impl<F: Field> CpuTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Table<F> {
        Table::new(TableKind::Cpu, columns, filter_column)
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

pub trait Lookups<F: Field> {
    fn lookups() -> CrossTableLookup<F>;
}

pub struct RangecheckCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for RangecheckCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_rangecheck(),
                Some(cpu::columns::filter_for_rangecheck()),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                Some(rangecheck::columns::filter_for_cpu()),
            ),
        )
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
                let filter = if let Some(column) = &table.filter_column {
                    column.eval_table(trace, i)
                } else {
                    F::ONE
                };
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
                looking_tables: vec![CpuTable::new(lookup_data(&[1]), Some(lookup_filter(2)))],
                looked_table: RangeCheckTable::new(lookup_data(&[1]), Some(lookup_filter(0))),
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
                looking_tables: vec![CpuTable::new(lookup_data(&[0]), Some(lookup_filter(0)))],
                looked_table: RangeCheckTable::new(lookup_data(&[1]), Some(lookup_filter(0))),
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
