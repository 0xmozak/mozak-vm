use mozak_runner::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut memory_inits: Vec<MemoryInit<F>> =
        [(F::ZERO, &program.ro_memory), (F::ONE, &program.rw_memory)]
            .iter()
            .flat_map(|&(is_writable, mem)| {
                mem.iter().map(move |(&addr, &value)| MemoryInit {
                    filter: F::ONE,
                    is_writable,
                    element: MemElement {
                        address: F::from_canonical_u32(addr),
                        value: F::from_canonical_u8(value),
                    },
                })
            })
            .collect();

    memory_inits.sort_by_key(|init| {
        let addr: u64 = init.element.address.to_canonical_u64();
        addr
    });

    pad_trace_with_default(memory_inits)
}
