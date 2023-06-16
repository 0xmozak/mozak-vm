use plonky2::field::types::Field;

/// Represent a linear combination of columns.
#[derive(Clone, Debug)]
pub struct Column<F: Field> {
    linear_combination: Vec<(usize, F)>,
    constant: F,
}

#[derive(Clone, Debug)]
pub struct Table<F: Field> {
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
