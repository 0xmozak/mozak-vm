use log::debug;
use p3_field::Field;
use p3_matrix::dense::RowMajorMatrix;

use crate::columns_view::NumberOfColumns;
use crate::xor::columns::{XorColumnsView, XorView};
fn to_bits(n: u32) -> [u32; 32] {
    let mut bits = [0; 32];
    for i in 0..32 {
        bits[i] = (n >> i) & 1;
    }
    bits
}
pub fn generate_dummy_xor_trace<F: Field>(n: usize) -> RowMajorMatrix<F> {
    let n = n as u32;
    let num_rows = n.next_power_of_two() as u32;
    let trace_values = (0..n).flat_map(|i: u32| XorColumnsView {
        is_execution_row: 1,
        execution: XorView {
            a: i,
            b: i.wrapping_add(1),
            out: i ^ (i.wrapping_add(1)),
        },
        limbs: XorView {
            a: to_bits(i),
            b: to_bits(i.wrapping_add(1)),
            out: to_bits(i ^ (i.wrapping_add(1))),
        },
    });
    let pad_values = (n..num_rows).flat_map(|_| XorColumnsView::default());

    let trace = trace_values
        .chain(pad_values)
        .map(F::from_canonical_u32)
        .collect();
    debug!("{trace:?}");
    RowMajorMatrix::new(trace, XorColumnsView::<()>::NUMBER_OF_COLUMNS)
}
