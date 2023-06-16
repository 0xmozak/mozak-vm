use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{COL_DST_VALUE, COL_IMM_VALUE, COL_PC, COL_S_LUI, NUM_CPU_COLS};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Since while decoding immediate we set 12 LSB to 0
    // VM puts same value to destination
    yield_constr.constraint(lv[COL_S_LUI] * (lv[COL_DST_VALUE] - lv[COL_IMM_VALUE]));

    // pc ticks up
    let inc: P = column_of_xs(4_u32);
    yield_constr.constraint_transition((lv[COL_S_LUI]) * (nv[COL_PC] - lv[COL_PC] - inc));
}
