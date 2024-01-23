use itertools::{chain, Itertools};
use mozak_runner::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::{MemElement, MemoryInit};
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut memory_inits: Vec<MemoryInit<F>> = chain! {
        elf_memory_init(program),
        mozak_memory_init(program),
    }
    .collect();

    memory_inits.sort_by_key(|init| init.element.address.to_canonical_u64());

    let trace = pad_trace_with_default(memory_inits);
    log::trace!("MemoryInit trace {:?}", trace);
    trace
}

/// Generates a mozak memory init ROM trace
#[must_use]
pub fn generate_mozak_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    // TODO(Roman): we need to introduce in the following PR, new constraint that
    // insures that we only have SINGLE memory init per each address. This way we
    // force for memory-addresses not to overlap. For example: imaging someone
    // compile ELF with modified version of mozak-linker-script, and then also make
    // use of buggy mozak-loader code that does not insures non-overlapping nature
    // of the elf-ro & mozak-ro memory regions -> this new constraint will insure
    // that this situation will be properly handled
    let mut memory_inits: Vec<MemoryInit<F>> = mozak_memory_init(program);
    memory_inits.sort_by_key(|init| init.element.address.to_canonical_u64());

    let trace = pad_trace_with_default(memory_inits);
    log::trace!("MozakMemoryInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn mozak_memory_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    program
        .mozak_ro_memory
        .iter()
        .flat_map(|mozak_ro_memory| {
            // TODO(Roman): once context variables will be in use, extend it here !
            chain! {
                mozak_ro_memory.io_tape_public.data.iter(),
                mozak_ro_memory.io_tape_private.data.iter(),
            }
        })
        .map(|(&addr, &value)| MemoryInit {
            filter: F::ONE,
            is_writable: F::ZERO,
            element: MemElement {
                address: F::from_canonical_u32(addr),
                value: F::from_canonical_u8(value),
            },
        })
        .collect_vec()
}

#[must_use]
pub fn elf_memory_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
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
        .collect_vec()
}

// TODO(Roman): rename it to `generate_elf_memory_init_trace`
#[must_use]
pub fn generate_elf_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut memory_inits: Vec<MemoryInit<F>> = elf_memory_init(program);
    memory_inits.sort_by_key(|init| init.element.address.to_canonical_u64());

    let trace = pad_trace_with_default(memory_inits);
    log::trace!("ElfMemoryInit trace {:?}", trace);
    trace
}
