use log::debug;
use p3_field::Field;
use p3_matrix::dense::RowMajorMatrix;

use crate::columns_view::NumberOfColumns;
use crate::cpu::columns::Add;

#[must_use]
pub fn generate_add_trace<F: Field>(op1: u32, op2: u32, out: u32) -> RowMajorMatrix<F> {
    let op1_le_bytes = op1.to_le_bytes();
    let op2_le_bytes = op2.to_le_bytes();
    let out_le_bytes = out.to_le_bytes();
    let mut add = Add::<F> {
        op1: op1_le_bytes.map(|x| F::from_canonical_u8(x)),
        op2: op2_le_bytes.map(|x| F::from_canonical_u8(x)),
        out: out_le_bytes.map(|x| F::from_canonical_u8(x)),
        ..Default::default()
    };
    let mut carry_1 = 0;
    let mut carry_2 = 0;
    if u32::from(op1_le_bytes[0]) + u32::from(op2_le_bytes[0]) > 255 {
        carry_1 = 1;
        add.carry[0] = F::one();
    }
    if u32::from(op1_le_bytes[1]) + u32::from(op2_le_bytes[1]) + carry_1 > 255 {
        carry_2 = 1;
        add.carry[1] = F::one();
    }
    if u32::from(op1_le_bytes[2]) + u32::from(op2_le_bytes[2]) + carry_2 > 255 {
        add.carry[2] = F::one();
    }
    // add one defult to make row count 2
    let trace = add.into_iter().chain(Add::default()).collect();
    debug!("{trace:?}");
    RowMajorMatrix::new(trace, Add::<()>::NUMBER_OF_COLUMNS)
}
