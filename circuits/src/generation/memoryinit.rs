use mozak_runner::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut combined_memory: Vec<(F, u32, u8)> = Vec::new();

    for (is_writable, mem) in &[(F::ZERO, &program.ro_memory), (F::ONE, &program.rw_memory)] {
        let mut sorted_mem: Vec<(&u32, &u8)> = mem.iter().collect();
        sorted_mem.sort_by_key(|&(addr, _)| *addr);
        for (&addr, &value) in sorted_mem.iter() {
            combined_memory.push((is_writable.clone(), addr, value));
        }
    }

    let trace: Vec<MemoryInit<F>> = combined_memory.into_iter().map(|(is_writable, addr, value)| MemoryInit {
        filter: F::ONE,
        is_writable,
        element: MemElement {
            address: F::from_canonical_u32(addr),
            value: F::from_canonical_u8(value),
        },
    }).collect();

    pad_trace_with_default(trace)
}
