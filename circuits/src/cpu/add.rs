use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_PC, COL_S_ADD, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at: P = column_of_xs(1_u64 << 32);
    let added = lv[COL_OP1_VALUE] + lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let wrapped = added - wrap_at;

    yield_constr
        .constraint(lv[COL_S_ADD] * (lv[COL_DST_VALUE] - added) * (lv[COL_DST_VALUE] - wrapped));

    // pc ticks up
    // TODO(Matthias): factor this out into a function to be used by most
    // instruction, ie all that are not jumping or branching.
    // NOTE(Matthias): if we are careful, bumping the pc by 4 does not need a range
    // check, because we can statically guarantee that the PC is far from
    // wrapping around in both field and u32.
    let inc: P = column_of_xs(4_u32);
    yield_constr.constraint_transition((lv[COL_S_ADD]) * (nv[COL_PC] - lv[COL_PC] - inc));
}
