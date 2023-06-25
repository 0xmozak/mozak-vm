use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_ADD, NUM_CPU_COLS,
};
use super::utils::pc_ticks_up;
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at: P = column_of_xs(1 << 32);
    let added = lv[COL_OP1_VALUE] + lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let wrapped = added - wrap_at;

    yield_constr
        .constraint(lv[COL_S_ADD] * (lv[COL_DST_VALUE] - added) * (lv[COL_DST_VALUE] - wrapped));

    yield_constr.constraint_transition((lv[COL_S_ADD]) * pc_ticks_up(lv, nv));
}
