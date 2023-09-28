use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_limb::columns::RangeCheckLimb;

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<RangeCheckLimb<F>>) -> Vec<RangeCheckLimb<F>> {
    let len = trace.len().next_power_of_two();
    trace.resize(len, RangeCheckLimb {
        filter: F::ZERO,
        element: F::from_canonical_u8(u8::MAX),
    });
    trace
}

#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    rangecheck_trace: &[RangeCheckColumnsView<F>],
) -> Vec<RangeCheckLimb<F>> {
    pad_trace(
        rangecheck_trace
            .iter()
            .filter(|row| row.filter.is_one())
            .flat_map(|row| &row.limbs)
            .map(F::to_canonical_u64)
            .sorted()
            .merge_join_by(0..=u64::from(u8::MAX), u64::cmp)
            .map(|value_or_dummy| {
                RangeCheckLimb {
                    filter: value_or_dummy.has_left().into(),
                    element: value_or_dummy.into_left(),
                }
                .map(F::from_noncanonical_u64)
            })
            .collect::<Vec<_>>(),
    )
}
