use plonky2::field::packed::PackedField;

use super::columns::{COL_PC, NUM_CPU_COLS};
use crate::utils::column_of_xs;

pub fn pc_ticks_up<P: PackedField>(lv: &[P; NUM_CPU_COLS], nv: &[P; NUM_CPU_COLS]) -> P {
    nv[COL_PC] - (lv[COL_PC] + column_of_xs::<P>(4))
}
