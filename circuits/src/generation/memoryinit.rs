use mozak_vm::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemoryInit, ROMemElement};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    pad_trace_with_default(
        program
            .data
            .iter()
            .map(|(&addr, &value)| MemoryInit {
                filter: F::ONE,
                rodata: ROMemElement::from([addr, u32::from(value)]).map(F::from_canonical_u32),
            })
            .collect(),
    )
}
