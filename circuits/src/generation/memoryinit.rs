use mozak_vm::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    pad_trace_with_default(
        [(F::ZERO, &program.ro_memory), (F::ONE, &program.rw_memory)]
            .into_iter()
            .flat_map(|(is_writable, mem)| {
                mem.iter().map(move |(&addr, &value)| MemoryInit {
                    filter: F::ONE,
                    is_writable,
                    element: MemElement {
                        address: F::from_canonical_u32(addr),
                        value: F::from_canonical_u8(value),
                    },
                })
            })
            .collect(),
    )
}
