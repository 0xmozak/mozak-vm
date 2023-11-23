use p3_field::Field;
use p3_matrix::dense::RowMajorMatrix;

use crate::bitshift::columns::BitShift;
use crate::columns_view::NumberOfColumns;

pub fn generate_bitshift_trace<F: Field>() -> RowMajorMatrix<F> {
    // find better way to extract the const
    const NUM_COLS: usize = BitShift::<usize>::NUMBER_OF_COLUMNS;
    let mut trace_values = Vec::with_capacity(NUM_COLS * 32);
    for i in 0..32 {
        let slice: [F; NUM_COLS] = BitShift {
            amount: i as u32,
            multiplier: 1 << i,
        }
        .map(F::from_canonical_u32)
        .into();
        trace_values.extend(slice);
    }
    RowMajorMatrix::new(trace_values, NUM_COLS)
}
