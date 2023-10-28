use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use super::rangecheck::extract;
use crate::cpu::columns::CpuState;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_limb::columns::RangeCheckLimb;
use crate::stark::mozak_stark::{LimbTable, Lookups, TableKind};

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<RangeCheckLimb<F>>) -> Vec<RangeCheckLimb<F>> {
    let len = trace.len().next_power_of_two().max(4);
    trace.resize(len, RangeCheckLimb {
        filter: F::ZERO,
        element: F::from_canonical_u8(u8::MAX),
    });
    trace
}

/// Generate a limb lookup trace from `rangecheck_trace`
#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    rangecheck_trace: &[RangeCheckColumnsView<F>],
) -> Vec<RangeCheckLimb<F>> {
    pad_trace(
        LimbTable::lookups()
            .looking_tables
            .into_iter()
            .flat_map(|looking_table| match looking_table.kind {
                TableKind::RangeCheck => extract(rangecheck_trace, &looking_table),
                TableKind::Cpu => extract(cpu_trace, &looking_table),
                other => unimplemented!("Can't range check {other:?} tables"),
            })
            .map(|limb| F::to_canonical_u64(&limb))
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
