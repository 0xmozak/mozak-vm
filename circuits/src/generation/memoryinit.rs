use itertools::{chain, Itertools};
use mozak_runner::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::memoryinit::columns::MemoryInit;
use crate::utils::pad_trace_with_default;

/// Generates a memory init ROM trace (ELF + Mozak)
#[must_use]
pub fn generate_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let mut memory_inits: Vec<MemoryInit<F>> = chain! {
        elf_memory_init(program),
        mozak_memory_init(program),
        call_tape_init(program),
        private_tape_init(program),
        public_tape_init(program),
        event_tape_init(program),
    }
    .collect();

    memory_inits.sort_by_key(|init| init.address.to_canonical_u64());

    let trace = pad_trace_with_default(memory_inits);
    log::trace!("MemoryInit trace {:?}", trace);
    trace
}

/// Generates a generic memory init trace sorted by address. Useful for memory
/// represented as
/// [`MozakMemoryRegion`](mozak_runner::elf::MozakMemoryRegion) traces.
pub fn generate_init_trace<F: RichField, Fn>(program: &Program, f: Fn) -> Vec<MemoryInit<F>>
where
    Fn: FnOnce(&Program) -> Vec<MemoryInit<F>>, {
    let mut memory_inits: Vec<MemoryInit<F>> = f(program);
    memory_inits.sort_by_key(|init| init.address.to_canonical_u64());

    pad_trace_with_default(memory_inits)
}

/// Generates a mozak memory init ROM trace
#[must_use]
pub fn generate_mozak_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let trace = generate_init_trace(program, mozak_memory_init);
    log::trace!("MozakMemoryInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn mozak_memory_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    program
        .mozak_ro_memory
        .iter()
        .flat_map(|mozak_ro_memory| {
            chain! {
                mozak_ro_memory.self_prog_id.data.clone(),
                mozak_ro_memory.cast_list.data.clone(),
            }
        })
        .map(MemoryInit::new_readonly)
        .collect_vec()
}

#[must_use]
pub fn call_tape_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    program
        .mozak_ro_memory
        .iter()
        .flat_map(|mozak_ro_memory| mozak_ro_memory.call_tape.data.clone())
        .map(MemoryInit::new_readonly)
        .collect_vec()
}

/// Generates a call tape memory init ROM trace
#[must_use]
pub fn generate_call_tape_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let trace = generate_init_trace(program, call_tape_init);
    log::trace!("CallTapeInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn private_tape_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    program
        .mozak_ro_memory
        .iter()
        .flat_map(|mozak_ro_memory| mozak_ro_memory.private_tape.data.clone())
        .map(MemoryInit::new_readonly)
        .collect_vec()
}

/// Generates a private tape memory init ROM trace
#[must_use]
pub fn generate_private_tape_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let trace = generate_init_trace(program, private_tape_init);
    log::trace!("PrivateTapeInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn public_tape_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    program
        .mozak_ro_memory
        .iter()
        .flat_map(|mozak_ro_memory| mozak_ro_memory.public_tape.data.clone())
        .map(MemoryInit::new_readonly)
        .collect_vec()
}

/// Generates a public tape memory init ROM trace
#[must_use]
pub fn generate_public_tape_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let trace = generate_init_trace(program, public_tape_init);
    log::trace!("PublicTapeInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn event_tape_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    program
        .mozak_ro_memory
        .iter()
        .flat_map(|mozak_ro_memory| mozak_ro_memory.event_tape.data.clone())
        .map(MemoryInit::new_readonly)
        .collect_vec()
}

/// Generates a event tape memory init ROM trace
#[must_use]
pub fn generate_event_tape_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let trace = generate_init_trace(program, event_tape_init);
    log::trace!("EventTapeInit trace {:?}", trace);
    trace
}

#[must_use]
pub fn elf_memory_init<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    [(F::ZERO, &program.ro_memory), (F::ONE, &program.rw_memory)]
        .iter()
        .flat_map(|&(is_writable, mem)| {
            mem.iter().map(move |(&addr, &value)| MemoryInit {
                filter: F::ONE,
                is_writable,
                address: F::from_canonical_u32(addr),
                value: F::from_canonical_u8(value),
            })
        })
        .collect_vec()
}

/// Generates a elf memory init ROM trace
#[must_use]
pub fn generate_elf_memory_init_trace<F: RichField>(program: &Program) -> Vec<MemoryInit<F>> {
    let trace = generate_init_trace(program, elf_memory_init);
    log::trace!("ElfMemoryInit trace {:?}", trace);
    trace
}
