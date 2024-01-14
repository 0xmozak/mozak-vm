use mozak_runner::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut memory_inits: Vec<MemoryInit<F>> =
        generate_memory_init_trace_without_mozak_ro_memory(program);
    // extend memory init with new io-tapes mechanism
    #[allow(clippy::unnecessary_operation)]
    let _ = &program.mozak_ro_memory.is_some().then(|| {
        let mozak_memory_inits: Vec<MemoryInit<F>> = [
            &program
                .mozak_ro_memory
                .as_ref()
                .unwrap()
                .io_tape_public
                .data,
            &program
                .mozak_ro_memory
                .as_ref()
                .unwrap()
                .io_tape_private
                .data,
        ]
        .iter()
        .flat_map(|mem| {
            mem.iter().map(move |(&addr, &value)| MemoryInit {
                filter: F::ONE,
                is_writable: F::ZERO,
                element: MemElement {
                    address: F::from_canonical_u32(addr),
                    value: F::from_canonical_u8(value),
                },
            })
        })
        .collect();

        memory_inits.extend(mozak_memory_inits.iter());
    });

    memory_inits.sort_by_key(|init| init.element.address.to_canonical_u64());

    let trace = pad_trace_with_default(memory_inits);
    log::trace!("MemoryInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn generate_memory_init_trace_without_mozak_ro_memory<F: RichField>(
    program: &Program,
) -> Vec<MemoryInit<F>> {
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
    memory_inits.sort_by_key(|init| init.element.address.to_canonical_u64());

    let trace = pad_trace_with_default(memory_inits);
    log::trace!("MemoryInit trace {:?}", trace);
    trace
}
