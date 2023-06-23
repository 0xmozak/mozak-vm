use std::borrow::Borrow;

use itertools::Itertools;
use plonky2::field::{polynomial::PolynomialValues, types::Field};

/// Represent a linear combination of columns.
#[derive(Clone, Debug)]
pub struct Column<F: Field> {
    linear_combination: Vec<(usize, F)>,
    constant: F,
}

impl<F: Field> Column<F> {
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
            .map(|&(c, f)| {
                println!(
                    "{} (table[{c}].values[{row:?}]) * {f} = {}",
                    table[c].values[row],
                    table[c].values[row] * f
                );

                table[c].values[row] * f
            })
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

pub struct RangeCheckTable<F: Field>(Table<F>);
pub struct CpuTable<F: Field>(Table<F>);

impl<F: Field> RangeCheckTable<F> {
    pub fn new(columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Table<F> {
        Table::new(TableKind::RangeCheck, columns, filter_column)
    }
}

impl<F: Field> CpuTable<F> {
    pub fn new(columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Table<F> {
        Table::new(TableKind::Cpu, columns, filter_column)
    }
}

#[derive(Clone)]
pub struct CrossTableLookup<F: Field> {
    pub(crate) looking_tables: Vec<Table<F>>,
    pub(crate) looked_table: Table<F>,
}

impl<F: Field> CrossTableLookup<F> {
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

// pub struct RangecheckCpuTable<F: Field>(CrossTableLookup<F>);
// impl<F: Field> Lookups<F> for RangecheckCpuTable<F> {
//     fn lookups() -> CrossTableLookup<F> {
//         CrossTableLookup::new(vec![], Table::new())
//     }
// }

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, u8};

    use plonky2::field::{goldilocks_field::GoldilocksField, polynomial::PolynomialValues};

    use super::*;

    type MultiSet<F> = HashMap<Vec<F>, Vec<(TableKind, usize)>>;

    struct FooBarTable<F: Field>(CrossTableLookup<F>);

    fn lookup_data<F: Field>(col_indices: &[usize]) -> Vec<Column<F>> {
        Column::singles(col_indices).collect_vec()
    }

    fn lookup_filter<F: Field>(col_idx: usize) -> Column<F> {
        Column::single(col_idx)
    }

    impl<F: Field> Lookups<F> for FooBarTable<F> {
        fn lookups() -> CrossTableLookup<F> {
            CrossTableLookup {
                looking_tables: vec![CpuTable::new(lookup_data(&[1]), Some(lookup_filter(0)))],
                looked_table: RangeCheckTable::new(lookup_data(&[1]), Some(lookup_filter(0))),
            }
        }
    }

    // impl<F: Field> Column<F> {
    //     fn rand(num_vals: usize) -> Self {
    //         Self {
    //             linear_combination: (),
    //             constant: (),
    //         }
    //     }
    // }

    // impl<F: Field> Table<F> {
    //     /// Fill a table with random values.
    //     fn dummy(kind: TableKind, num_cols: usize, filter_column:
    // Option<Column<F>>) -> Self {         let columns = vec![Column];
    //         Self {
    //             columns: columns,
    //             kind,
    //             filter_column,
    //         }
    //     }
    // }

    // Check that the provided trace and cross-table lookup are consistent.
    fn check_ctl<F: Field>(
        trace_poly_values: &[Vec<PolynomialValues<F>>],
        ctl: &CrossTableLookup<F>,
        ctl_index: usize,
    ) {
        // Maps `m` with `(table.kind, i) in m[row]` iff the `i`-th row of the table
        // is equal to `row` and the filter is 1.
        //
        // the CTL check holds iff `looking_multiset == looked_multiset`.
        let mut looking_multiset = MultiSet::<F>::new();
        let mut looked_multiset = MultiSet::<F>::new();

        for looking_table in &ctl.looking_tables {
            let trace = &trace_poly_values[looking_table.kind as usize];
            println!(
                "trace.len() {}, trace[0].len() {}",
                trace.len(),
                trace[0].len()
            );
            for i in 0..trace[0].len() {
                let filter = if let Some(column) = &looking_table.filter_column {
                    println!(
                        "filter: eval({:?}, {}): {:?}",
                        trace,
                        i,
                        column.eval_table(trace, i)
                    );
                    column.eval_table(trace, i)
                } else {
                    F::ONE
                };
                if filter.is_one() {
                    println!("looking_table.columns: {:?}", looking_table.columns);
                    let row = looking_table
                        .columns
                        .iter()
                        .map(|c| {
                            println!("c.eval_table(): {:?}", c.eval_table(trace, i));
                            c.eval_table(trace, i)
                        })
                        .collect::<Vec<_>>();
                    println!("row: {:?}", row);
                    looking_multiset
                        .entry(row)
                        .or_default()
                        .push((looking_table.kind, i));
                } else {
                    println!("filter: {}", filter);
                    assert_eq!(filter, F::ZERO, "Non-binary filter?")
                }
            }
        }

        let trace = &trace_poly_values[ctl.looked_table.kind as usize];
        for i in 0..trace[0].len() {
            let filter = if let Some(column) = &ctl.looked_table.filter_column {
                column.eval_table(trace, i)
            } else {
                F::ONE
            };
            if filter.is_one() {
                let row = ctl
                    .looked_table
                    .columns
                    .iter()
                    .map(|c| c.eval_table(trace, i))
                    .collect::<Vec<_>>();
                looked_multiset
                    .entry(row)
                    .or_default()
                    .push((ctl.looked_table.kind, i));
            } else {
                assert_eq!(filter, F::ZERO, "Non-binary filter?")
            }
        }
        println!("looked multiset: {:?}", looked_multiset);
        println!("looking multiset: {:?}", looking_multiset);

        let empty = &vec![];
        // Check that every row in the looking tables appears in the looked table the
        // same number of times.
        for (row, looking_locations) in &looking_multiset {
            let looked_locations = looked_multiset.get(row).unwrap_or(empty);
            println!("looked locations: {:?}", looked_locations);
            assert_eq!(looking_locations.len(), looked_locations.len(),
               "CTL #{ctl_index}:\n\
                 Row {row:?} is present {l0} times in the looking tables, but {l1} times in the looked table.\n\
                 Looking locations (Table, Row index): {looking_locations:?}.\n\
                 Looked locations (Table, Row index): {looked_locations:?}.",
                l0 = looking_locations.len(),
                l1 = looked_locations.len())
        }
        // Check that every row in the looked tables appears in the looking table the
        // same number of times.
        for (row, looked_locations) in &looked_multiset {
            let looking_locations = looking_multiset.get(row).unwrap_or(empty);
            assert_eq!(looking_locations.len(), looked_locations.len(),
               "CTL #{ctl_index}:\n\
                 Row {row:?} is present {l0} times in the looking tables, but {l1} times in the looked table.\n\
                 Looking locations (Table, Row index): {looking_locations:?}.\n\
                 Looked locations (Table, Row index): {looked_locations:?}.",
                l0 = looking_locations.len(),
                l1 = looked_locations.len())
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Trace<F: Field> {
        trace: Vec<PolynomialValues<F>>,
    }

    impl<F: Field> Trace<F> {
        pub fn builder() -> TraceBuilder<F> {
            TraceBuilder::default()
        }
    }

    #[derive(Default)]
    pub struct TraceBuilder<F: Field> {
        trace: Vec<PolynomialValues<F>>,
    }

    impl<F: Field> TraceBuilder<F> {
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

        pub fn num_rows(mut self, num_rows: usize) -> TraceBuilder<F> {
            for i in 0..self.trace.len() {
                let mut row = vec![];
                for _ in 0..num_rows {
                    row.push(F::rand())
                }
                self.trace.push(PolynomialValues::from(row));
            }
            self
        }

        pub fn zero(mut self, idx: usize) -> TraceBuilder<F> {
            self.trace[idx] = PolynomialValues::zero(self.trace[idx].len());

            self
        }

        pub fn one(mut self, idx: usize) -> TraceBuilder<F> {
            let len = self.trace[idx].len();
            let ones = PolynomialValues::constant(F::ONE, len);
            self.trace[idx] = ones;

            self
        }

        pub fn build(self) -> Vec<PolynomialValues<F>> {
            self.trace
        }
    }

    #[test]
    fn test_ctl() {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup<F> = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4).one(0).build();
        let bar_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4).one(0).build();
        let traces = vec![foo_trace, bar_trace];
        check_ctl(&traces, &dummy_cross_table_lookup, 0);
    }
}
