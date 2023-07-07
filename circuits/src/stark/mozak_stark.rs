use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;

use crate::cpu::stark::CpuStark;
#[derive(Clone, Default)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
}

pub(crate) const NUM_TABLES: usize = 1;
