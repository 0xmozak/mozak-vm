use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::*;
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint(
        lv[COL_S_ADD] * (lv[COL_DST_VALUE] - (lv[COL_OP1_VALUE] + lv[COL_OP2_VALUE] + lv[COL_IMM])),
    );

    // pc ticks up
    let inc: P = column_of_xs(4_u32);
    yield_constr.constraint((lv[COL_S_ADD]) * (nv[COL_PC] - lv[COL_PC] + inc));
}
