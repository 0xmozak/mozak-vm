use mozak_vm::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    pad_trace_with_default(
        program
            .data
            .iter()
            .map(|(&addr, &value)| MemoryInit {
                filter: F::ONE,
                rodata: MemElement {
                    address: F::from_canonical_u32(addr),
                    value: F::from_canonical_u8(value),
                },
            })
            .collect(),
    )
}
