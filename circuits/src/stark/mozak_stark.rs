use plonky2::{field::extension::Extendable, hash::hash_types::RichField};

use crate::cpu::cpu_stark::CpuStark;

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
        }
    }
}

pub(crate) const NUM_TABLES: usize = 1;
