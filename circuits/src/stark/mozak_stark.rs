use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{CrossTableLookup, Lookups, RangecheckCpuTable};
use crate::rangecheck::stark::RangeCheckStark;
#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 1],
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            rangecheck_stark: RangeCheckStark::default(),
            cross_table_lookups: [RangecheckCpuTable::lookups(); 1],
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.num_permutation_batches(config),
            self.rangecheck_stark.num_permutation_batches(config),
        ]
    }

    pub(crate) fn permutation_batch_sizes(&self) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.permutation_batch_size(),
            self.rangecheck_stark.permutation_batch_size(),
        ]
    }
}

pub(crate) const NUM_TABLES: usize = 2;
