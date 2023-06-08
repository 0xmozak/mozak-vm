use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::*;

pub(crate) fn eval_packed_generic<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    _nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr
        .constraint(lv[COL_S_ADD] * (lv[COL_DST_VALUE] - (lv[COL_OP1_VALUE] + lv[COL_OP2_VALUE])));
}
