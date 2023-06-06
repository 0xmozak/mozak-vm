use plonky2::{hash::hash_types::RichField, field::extension::Extendable};

use crate::cpu::cpu_stark::CpuStark;


#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self { cpu_stark: CpuStark::default() }
    }
}
