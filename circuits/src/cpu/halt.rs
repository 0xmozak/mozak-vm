use log::trace;
use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{COL_PC, COL_S_HALT, NUM_CPU_COLS};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // pc stays same
    yield_constr.constraint_transition(lv[COL_S_HALT] * (nv[COL_PC] - lv[COL_PC]));
}
