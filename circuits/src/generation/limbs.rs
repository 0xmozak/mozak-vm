use std::borrow::Borrow;

use itertools::{izip, Itertools};
use plonky2::hash::hash_types::RichField;

use crate::columns_view::NumberOfColumns;
use crate::limbs::columns::Limbs;
use crate::rangecheck::columns::RangeCheckColumnsView;

pub fn pad_trace<F: RichField>(mut trace: Vec<Limbs<F>>) -> Vec<Limbs<F>> {
    let len = trace.len().next_power_of_two();
    trace.resize(len, Limbs {
        filter: F::ZERO,
        range_check_u16: F::from_canonical_u16(u16::MAX),
    });
    trace
}

pub fn generate_limbs_trace<F: RichField>(
    rangecheck_trace: &[Vec<F>; RangeCheckColumnsView::<()>::NUMBER_OF_COLUMNS],
) -> Vec<Limbs<F>> {
    let r: &RangeCheckColumnsView<_> = rangecheck_trace.borrow();
    pad_trace(
        izip!(&r.limb_lo, &r.limb_hi, &r.filter)
            .filter(|(_, _, filter)| filter.is_one())
            .flat_map(|(lo, hi, _)| [lo, hi])
            .map(F::to_canonical_u64)
            .sorted()
            .merge_join_by(0..u16::MAX as u64, u64::cmp)
            .map(|value_or_dummy| {
                Limbs {
                    filter: value_or_dummy.has_left().into(),
                    range_check_u16: value_or_dummy.into_left().into(),
                }
                .map(F::from_noncanonical_u64)
            })
            .collect::<Vec<_>>(),
    )
}
