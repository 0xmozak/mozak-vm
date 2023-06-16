use plonky2::field::types::Field;

use crate::cpu::stark::CpuStark;

#[derive(Clone, Default)]
pub struct MozakStark<F: Field> {
    pub cpu_stark: CpuStark<F>,
}

pub(crate) const NUM_TABLES: usize = 1;
