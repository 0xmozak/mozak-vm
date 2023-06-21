use plonky2::{field::extension::Extendable, hash::hash_types::RichField};

use crate::cpu::stark::CpuStark;
#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    // pub cross_table_lookups: [CrossTableLookup<F>; 1],
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            // cross_table_lookups: [RangecheckCpuTable::lookups(); 1],
        }
    }
}

pub(crate) const NUM_TABLES: usize = 1;
