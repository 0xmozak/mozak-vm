use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::*;

pub(crate) fn eval_packed_generic<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // pc stays same
    yield_constr.constraint(lv[COL_S_HALT] * (nv[COL_PC] - lv[COL_PC]));
}
