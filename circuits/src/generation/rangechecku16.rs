use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_u16::columns::RangeCheckU16;

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<RangeCheckU16<F>>) -> Vec<RangeCheckU16<F>> {
    let len = trace.len().next_power_of_two();
    trace.resize(len, RangeCheckU16 {
        filter: F::ZERO,
        range_check_u16: F::from_canonical_u16(u16::MAX),
    });
    trace
}

#[must_use]
pub(crate) fn generate_rangechecku16_trace<F: RichField>(
    rangecheck_trace: &[RangeCheckColumnsView<F>],
) -> Vec<RangeCheckU16<F>> {
    pad_trace(
        rangecheck_trace
            .iter()
            .filter(|row| row.filter.is_one())
            .flat_map(|row| [&row.limb_lo, &row.limb_hi])
            .map(F::to_canonical_u64)
            .sorted()
            .merge_join_by(0..=u64::from(u16::MAX), u64::cmp)
            .map(|value_or_dummy| {
                RangeCheckU16 {
                    filter: value_or_dummy.has_left().into(),
                    range_check_u16: value_or_dummy.into_left(),
                }
                .map(F::from_noncanonical_u64)
            })
            .collect::<Vec<_>>(),
    )
}
