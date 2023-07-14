use std::ops::Range;

use itertools::Itertools;
use plonky2::field::types::Field;

use crate::cross_table_lookup::Column;

/// Column containing the value (in u32) to be range checked.
pub(crate) const VAL: usize = 0;

/// Total number of values to be range checked. This value determines the number
/// of [`LIMBS`] columns.
pub(crate) const NUM_VALUES_TO_RANGECHECK: usize = VAL + 1;

/// Column to indicate that a value to be range checked is from the CPU table.
pub(crate) const CPU_FILTER: usize = VAL + 1;

/// We need 4 columns for each value to be range checked:
/// 1) Limb for hi bits,
/// 2) Limb for lo bits,
/// 3) Permuted limb for hi bits,
/// 4) Permuted limb for lo bits.
pub(crate) const LIMBS: Range<usize> =
    CPU_FILTER + 1..((VAL + 1) * 4 * NUM_VALUES_TO_RANGECHECK) + 1;

/// Offset into [`LIMBS`] range.
pub(crate) enum LimbKind {
    Hi = 0,
    Lo = 1,
    HiPermuted = 2,
    LoPermuted = 3,
}

impl LimbKind {
    pub fn col(value_idx: usize, limb: LimbKind) -> usize {
        let offset = LimbKind::LoPermuted as usize * value_idx;
        match limb {
            Self::Hi => LIMBS.start + offset,
            Self::Lo => LIMBS.start + offset + Self::Lo as usize,
            Self::HiPermuted => LIMBS.start + offset + Self::HiPermuted as usize,
            Self::LoPermuted => LIMBS.start + offset + Self::LoPermuted as usize,
        }
    }
}

/// Fixed column containing values 0, 1, .., 2^16 - 1.
pub(crate) const FIXED_RANGE_CHECK_U16: usize = LIMBS.end + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
/// fixed table lookup argument for the lower 16-bit limb.
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_LO: usize = FIXED_RANGE_CHECK_U16 + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
/// fixed table lookup argument for the upper 16-bit limb.
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_HI: usize = FIXED_RANGE_CHECK_U16_PERMUTED_LO + 1;

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = FIXED_RANGE_CHECK_U16_PERMUTED_HI + 1;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles([VAL]).collect_vec() }

/// Column for a binary filter to indicate a range check from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(CPU_FILTER) }
