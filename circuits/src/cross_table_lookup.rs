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
pub struct Table<F: Field> {
    kind: TableKind,
    columns: Vec<Column<F>>,
    pub(crate) filter_column: Option<Column<F>>,
}

#[derive(Clone)]
pub struct CrossTableLookup<F: Field> {
    pub(crate) looking_tables: Vec<Table<F>>,
    pub(crate) looked_table: Table<F>,
}

impl<F: Field> CrossTableLookup<F> {
    pub fn new(looking_tables: Vec<Table<F>>, looked_table: Table<F>) -> Self {
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
    use std::collections::HashMap;

    use plonky2::field::{goldilocks_field::GoldilocksField, polynomial::PolynomialValues};

    use super::*;

    type MultiSet<F> = HashMap<Vec<F>, Vec<(TableKind, usize)>>;

    struct FooBarTable<F: Field>(CrossTableLookup<F>);

    enum TableKind {
        Foo = 0,
        Bar = 1,
    }

    // impl<F: Field> Lookups<F> for FooBarTable<F> {
    //     fn lookups() -> CrossTableLookup<F> {
    //         CrossTableLookup {
    //             looking_tables: vec![],
    //             looked_table: Table {
    //                 kind: TableKind::Foo,
    //                 columns: (),
    //                 filter_column: (),
    //             },
    //         }
    //     }
    // }

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

    fn process_table<F: Field>(
        trace: &[PolynomialValues<F>],
        table: &Table<F>,
        multiset: &mut MultiSet<F>,
    ) {
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
                // multiset.entry(row).or_default().push((table.kind, i));
            } else {
                assert_eq!(filter, F::ZERO, "Non-binary filter?")
            }
        }
    }

    // Check that the provided trace and cross-table lookup are consistent.
    fn check_ctl<F: Field>(
        trace_poly_values: &[Vec<PolynomialValues<F>>],
        ctl: &CrossTableLookup<F>,
        ctl_index: usize,
    ) {
        // Maps `m` with `(table, i) in m[row]` iff the `i`-th row of `table`
        // is equal to `row` and the filter is 1. Without default values,
        // the CTL check holds iff `looking_multiset == looked_multiset`.
        let mut looking_multiset = MultiSet::<F>::new();
        let mut looked_multiset = MultiSet::<F>::new();

        for looking_table in &ctl.looking_tables {
            let trace = &trace_poly_values[looking_table.kind as usize];
            for i in 0..trace[0].len() {
                let filter = if let Some(column) = &looking_table.filter_column {
                    column.eval_table(trace, i)
                } else {
                    F::ONE
                };
                if filter.is_one() {
                    let row = looking_table
                        .columns
                        .iter()
                        .map(|c| c.eval_table(trace, i))
                        .collect::<Vec<_>>();
                    // looking_multiset
                    //     .entry(row)
                    //     .or_default()
                    //     .push((looking_table.kind, i));
                } else {
                    assert_eq!(filter, F::ZERO, "Non-binary filter?")
                }
            }
        }
    }

    fn dummy_trace<F: Field>(num_cols: usize, num_values: usize) -> Vec<PolynomialValues<F>> {
        let mut poly_values = vec![];
        for i in 0..num_cols {
            let mut values = Vec::with_capacity(num_values);
            for j in 0..num_values {
                values.push(F::rand());
            }
            poly_values.push(PolynomialValues::from(values));
        }

        return poly_values;
    }

    #[test]
    fn test_ctl() {
        type F = GoldilocksField;
        // let dummy_cross_table_lookup: CrossTableLookup<F> = FooBarTable::lookups();

        let num_cols = 3;
        let num_values = 4;
        let foo_trace = dummy_trace::<GoldilocksField>(num_cols, num_values);
        let bar_trace = dummy_trace::<GoldilocksField>(num_cols, num_values);
        let traces = vec![foo_trace, bar_trace];
        // check_ctl(&traces, &dummy_cross_table_lookup, 0);
    }
}
