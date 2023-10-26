use std::collections::HashMap;

use plonky2::hash::hash_types::RichField;

use super::rangecheck::extract;
use crate::cpu::columns::CpuState;
use crate::rangecheck::columns::{MultiplicityView, RangeCheckColumnsView};
use crate::rangecheck_limb::columns::RangeCheckLimb;
use crate::stark::lookup::rangechecks_u8;
use crate::stark::mozak_stark::TableKind;
use crate::stark::utils::transpose_trace;

#[must_use]
pub(crate) fn generate_rangecheck_limb_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    rangecheck_limbs_trace: &[RangeCheckColumnsView<F>],
) -> Vec<Vec<F>> {
    let mut multiplicities: HashMap<u8, u8> = HashMap::new();

    rangechecks_u8()
        .looking_tables
        .into_iter()
        .for_each(|looking_table| {
            match looking_table.kind {
                TableKind::RangeCheck => rangecheck_limbs_trace
                    .iter()
                    .flat_map(|l| l.limbs.iter().map(|l| l).collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                // TableKind::Cpu => extract(cpu_trace, &looking_table),
                other => unimplemented!("Can't range check {other:?} tables"),
            }
            .into_iter()
            .for_each(|v| {
                println!("v: {v:?}");
                let value = u8::try_from(v.to_canonical_u64())
                    .expect("casting value to u32 should succeed");

                if let Some(x) = multiplicities.get_mut(&value) {
                    *x += 1;
                } else {
                    multiplicities.insert(value, 1);
                }
            });
        });
    let mut trace: Vec<RangeCheckLimb<F>> = Vec::with_capacity(multiplicities.len());

    for (i, (value, multiplicity)) in multiplicities.into_iter().enumerate() {
        trace.push(
            RangeCheckLimb {
                filter: 1,
                logup_u8: MultiplicityView {
                    value,
                    multiplicity,
                },
            }
            .map(F::from_canonical_u8),
        );
    }
    trace.resize(trace.len().next_power_of_two(), RangeCheckLimb::default());

    let trace = transpose_trace(trace);
    trace
}
