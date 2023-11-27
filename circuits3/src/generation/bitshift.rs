use log::debug;
use p3_field::Field;
use p3_matrix::dense::RowMajorMatrix;

use crate::bitshift::columns::BitShift;
use crate::columns_view::NumberOfColumns;

pub fn generate_bitshift_trace<F: Field>() -> RowMajorMatrix<F> {
    // find better way to extract the const
    let trace_values: Vec<F> = (0..32)
        .flat_map(|i| {
            BitShift {
                amount: i,
                multiplier: 1 << i,
            }
        })
        .map(F::from_canonical_u32)
        .collect();
    debug!("{trace_values:?}");
    RowMajorMatrix::new(trace_values, BitShift::<usize>::NUMBER_OF_COLUMNS)
}
