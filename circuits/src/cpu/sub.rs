use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_PC, COL_S_SUB, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at: P = column_of_xs(1_u64 << 32);
    let expecte_value = lv[COL_OP1_VALUE] - lv[COL_OP2_VALUE];
    let wrapped = wrap_at + expecte_value;
    yield_constr.constraint(
        lv[COL_S_SUB] * ((lv[COL_DST_VALUE] - expecte_value) * (lv[COL_DST_VALUE] - wrapped)),
    );

    // pc ticks up
    let inc: P = column_of_xs(4_u32);
    yield_constr.constraint_transition((lv[COL_S_SUB]) * (nv[COL_PC] - lv[COL_PC] - inc));
}
