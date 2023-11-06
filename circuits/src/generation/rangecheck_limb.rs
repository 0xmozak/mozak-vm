use std::collections::HashMap;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use super::rangecheck::extract;
use crate::cpu::columns::CpuState;
use crate::multiplicity_view::MultiplicityView;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_limb::columns::RangeCheckLimb;
use crate::stark::mozak_stark::{LimbTable, Lookups, TableKind};

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<RangeCheckLimb<F>>) -> Vec<RangeCheckLimb<F>> {
    let len = trace.len().next_power_of_two().max(4);
    trace.resize(len, RangeCheckLimb {
        filter: F::ZERO,
        element: F::from_canonical_u8(u8::MAX),
        multiplicity_view: MultiplicityView::default(),
    });
    trace
}

#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    rangecheck_trace: &[RangeCheckColumnsView<F>],
) -> Vec<RangeCheckLimb<F>> {
    let mut multiplicities: HashMap<u8, u64> = HashMap::new();

    let mut trace = pad_trace(
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
                let filter = u64::from(value_or_dummy.has_left());
                let val = value_or_dummy.into_left();
                if let Some(x) = multiplicities.get_mut(&u8::try_from(val).unwrap()) {
                    *x += 1;
                } else {
                    multiplicities.insert(val.try_into().unwrap(), 1);
                };

                RangeCheckLimb {
                    filter,
                    element: val,
                    multiplicity_view: MultiplicityView::default(),
                }
                .map(F::from_noncanonical_u64)
            })
            .collect::<Vec<_>>(),
    );

    for (i, (value, multiplicity)) in multiplicities.into_iter().enumerate() {
        trace[i].multiplicity_view.value = F::from_canonical_u8(value);
        trace[i].multiplicity_view.multiplicity = F::from_canonical_u64(multiplicity);
    }

    trace
}
