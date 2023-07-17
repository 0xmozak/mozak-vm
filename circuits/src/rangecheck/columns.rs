use std::ops::Range;

use itertools::Itertools;
use plonky2::field::types::Field;

use crate::cross_table_lookup::Column;

/// Column containing the value (in u32) to be range checked.
pub(crate) const VAL: usize = 0;

/// Column containing the value (in u32) to be range checked.
pub(crate) const OP1_FIXED: usize = VAL + 1;

/// Total number of values to be range checked. This value determines the number
/// of [`LIMBS`] columns.
pub(crate) const NUM_VALUES_TO_RANGECHECK: usize = OP1_FIXED + 1;

/// Column to indicate that a value to be range checked is from the CPU table.
pub(crate) const CPU_ADD: usize = OP1_FIXED + 1;

pub(crate) const FILTER_START: usize = CPU_ADD;

/// Column to indicate that a value to be range checked is from the CPU table.
pub(crate) const CPU_SLT: usize = CPU_ADD + 1;

/// We need 6 columns for each value to be range checked:
/// 1) Limb for hi bits,
/// 2) Limb for lo bits,
/// 3) Permuted limb for hi bits,
/// 4) Permuted limb for lo bits.
/// 5) Permuted limb for table (hi).
/// 6) Permuted limb for table (lo).
pub(crate) const LIMBS: Range<usize> = CPU_SLT + 1
    ..(CPU_SLT + 1 + (LimbKind::LoFixedPermuted as usize + 1) * NUM_VALUES_TO_RANGECHECK);

/// Offset into [`LIMBS`] range.
pub(crate) enum LimbKind {
    Hi = 0,
    Lo = 1,
    HiPermuted = 2,
    LoPermuted = 3,
    HiFixedPermuted = 4,
    LoFixedPermuted = 5,
}

impl LimbKind {
    pub fn col(value_idx: usize, limb: LimbKind) -> usize {
        let offset = (LimbKind::LoFixedPermuted as usize + 1) * value_idx;
        match limb {
            Self::Hi => LIMBS.start + offset,
            Self::Lo => LIMBS.start + offset + Self::Lo as usize,
            Self::HiPermuted => LIMBS.start + offset + Self::HiPermuted as usize,
            Self::LoPermuted => LIMBS.start + offset + Self::LoPermuted as usize,
            Self::HiFixedPermuted => LIMBS.start + offset + Self::HiFixedPermuted as usize,
            Self::LoFixedPermuted => LIMBS.start + offset + Self::LoFixedPermuted as usize,
        }
    }
}

/// Fixed column containing values 0, 1, .., 2^16 - 1.
pub(crate) const FIXED_RANGE_CHECK_U16: usize = LIMBS.end;

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = FIXED_RANGE_CHECK_U16 + 1;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles([VAL]).collect_vec() }

/// Column for a binary filter to indicate a range check from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(CPU_ADD) }
