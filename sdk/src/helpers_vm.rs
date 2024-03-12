// This file contains code snippets used in mozakvm execution

use std::ptr::{addr_of, slice_from_raw_parts};

use crate::linker_symbols::mozak_self_prog_id;
use crate::types::{Poseidon2HashType, ProgramIdentifier, DIGEST_BYTES};

/// Zero-copy archived format derivation of any given type (rkyv)
/// on a memory region starting at `addr`. It is expected that
/// the memory region is length-prefixed in little-endian 4-bytes
/// and (addr+4) marks the begining of the archived format.
pub fn archived_repr<T: rkyv::Archive>(addr: *const u8) -> &'static <T as rkyv::Archive>::Archived {
    let mem_len = unsafe { *{ addr as *const u32 } } as usize;
    unsafe {
        let mem_slice = &*slice_from_raw_parts::<u8>(addr.add(4), mem_len);
        rkyv::archived_root::<T>(mem_slice)
    }
}

/// Get the Program Identifier of the running program, assumes
/// pre-populated memory region starting `mozak_self_prog_id`.
pub fn get_self_prog_id() -> ProgramIdentifier {
    let self_prog_id = unsafe { *{ addr_of!(mozak_self_prog_id) as *const ProgramIdentifier } };
    assert_ne!(self_prog_id, ProgramIdentifier::default());
    self_prog_id
}

/// Hashes the input slice to `Poseidon2HashType`
#[allow(dead_code)]
pub fn poseidon2_hash(input: &[u8]) -> Poseidon2HashType {
    let mut padded_input = input.to_vec();
    // Why?
    padded_input.push(1);

    pub const RATE: usize = 8;

    padded_input.resize(padded_input.len().next_multiple_of(RATE), 0);

    let mut output = [0; DIGEST_BYTES];
    mozak_system::system::syscall_poseidon2(
        padded_input.as_ptr(),
        padded_input.len(),
        output.as_mut_ptr(),
    );
    Poseidon2HashType(output)
}
