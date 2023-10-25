use std::collections::HashMap;

use plonky2::hash::hash_types::RichField;

use super::rangecheck::extract;
use crate::cpu::columns::CpuState;
use crate::rangecheck_limb::columns::{RangeCheckLimb, MAP};
use crate::stark::mozak_stark::{LimbTable, Lookups, TableKind};
use crate::stark::utils::transpose_trace;

#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    rangecheck_limb_trace: &[Vec<F>],
) -> Vec<Vec<F>> {
    let mut multiplicities: HashMap<u8, u8> = HashMap::new();
    let mut trace: Vec<RangeCheckLimb<F>> =
        [0; 255].iter().map(|_| RangeCheckLimb::default()).collect();

    LimbTable::lookups()
        .looking_tables
        .into_iter()
        .for_each(|looking_table| {
            match looking_table.kind {
                TableKind::RangeCheck => extract(rangecheck_limb_trace, &looking_table),
                TableKind::Cpu => extract(cpu_trace, &looking_table),
                other => unimplemented!("Can't range check {other:?} tables"),
            }
            .into_iter()
            .for_each(|v| {
                let value = u8::try_from(v.to_canonical_u64())
                    .expect("casting value to u32 should succeed");

                if let Some(x) = multiplicities.get_mut(&value) {
                    *x += 1;
                } else {
                    multiplicities.insert(value, 1);
                }

                let row = RangeCheckLimb {
                    value,
                    filter: 1,
                    ..Default::default()
                }
                .map(F::from_canonical_u8);

                trace.push(row);
            });
        });
    let extension_len = trace.len().next_power_of_two() - trace.len();
    trace.resize(trace.len().next_power_of_two(), RangeCheckLimb::default());
    multiplicities.insert(
        0,
        multiplicities.get(&0).unwrap_or(&0) + u8::try_from(extension_len).unwrap(),
    );

    let mut trace = transpose_trace(trace);

    for (i, (value, multiplicity)) in multiplicities.iter().enumerate() {
        trace[MAP.logup_u8.value][i] = F::from_canonical_u8(*value);
        trace[MAP.logup_u8.multiplicity][i] = F::from_canonical_u8(*multiplicity);
    }

    trace
}
