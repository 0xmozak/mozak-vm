use mozak_vm::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut mem_trace: Vec<MemoryInit<F>> =
        Vec::with_capacity(program.ro_memory.len() + program.rw_memory.len());
    mem_trace.extend::<Vec<MemoryInit<F>>>(
        program
            .ro_memory
            .iter()
            .map(|(&addr, &value)| MemoryInit {
                filter: F::ONE,
                is_writable: F::ZERO,
                element: MemElement {
                    address: F::from_canonical_u32(addr),
                    value: F::from_canonical_u8(value),
                },
            })
            .collect(),
    );
    mem_trace.extend::<Vec<MemoryInit<F>>>(
        program
            .rw_memory
            .iter()
            .map(|(&addr, &value)| MemoryInit {
                filter: F::ONE,
                is_writable: F::ONE,
                element: MemElement {
                    address: F::from_canonical_u32(addr),
                    value: F::from_canonical_u8(value),
                },
            })
            .collect(),
    );
    pad_trace_with_default(mem_trace)
}
