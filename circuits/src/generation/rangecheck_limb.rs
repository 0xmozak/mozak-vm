use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::rangecheck::columns::RangeCheckColumnsView;
use crate::rangecheck_limb::columns::RangeCheckLimb;
use crate::stark::mozak_stark::{LimbTable, Lookups, Table, TableKind};

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<RangeCheckLimb<F>>) -> Vec<RangeCheckLimb<F>> {
    let len = trace.len().next_power_of_two();
    trace.resize(len, RangeCheckLimb {
        filter: F::ZERO,
        element: F::from_canonical_u8(u8::MAX),
    });
    trace
}

pub fn extract_u8<'a, F: RichField, V>(trace: &[V], looking_table: &Table<F>) -> Vec<F>
where
    V: Index<usize, Output = F> + 'a, {
    if let [column] = &looking_table.columns[..] {
        trace
            .iter()
            .filter(|&row| looking_table.filter_column.eval(row).is_one())
            .map(|row| {
                let val: F = column.eval(row);
                assert!(u8::try_from(val.to_canonical_u64()).is_ok());
                val
            })
            .collect()
    } else {
        panic!("Can only range check single values, not tuples.")
    }
}

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
                TableKind::RangeCheck => extract_u8(rangecheck_trace, &looking_table),
                TableKind::Cpu => extract_u8(cpu_trace, &looking_table),
                other => unimplemented!("Can't range check {other:#?} tables"),
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
