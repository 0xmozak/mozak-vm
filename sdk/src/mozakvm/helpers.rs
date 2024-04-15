// This file contains code snippets used in mozakvm execution

use std::ptr::{addr_of, slice_from_raw_parts};

use poseidon2::mozak_poseidon2;

use crate::common::types::poseidon2hash::DIGEST_BYTES;
use crate::common::types::{Poseidon2Hash, ProgramIdentifier};
use crate::mozakvm::linker_symbols::_mozak_self_prog_id;
/// Get a owned reference to a length-prefixed memory region.
/// It is expected that the memory region is length-prefixed
/// in little-endian 4-bytes and (addr+4) marks the begining
/// of the data.
#[allow(clippy::ptr_as_ptr)]
#[allow(clippy::cast_ptr_alignment)]
#[allow(clippy::ptr_cast_constness)]
pub fn owned_buffer(addr: *const u8) -> Box<[u8]> {
    let mem_len = unsafe { *{ addr as *const u32 } } as usize;
    unsafe {
        let mem_slice = slice_from_raw_parts::<u8>(addr.add(4), mem_len);
        Box::from_raw(mem_slice as *mut [u8])
    }
}

/// Zero-copy archived format derivation of any given type (rkyv)
/// on a memory region starting at `addr`. It is expected that
/// the memory region is length-prefixed in little-endian 4-bytes
/// and (addr+4) marks the begining of the archived format.
#[allow(clippy::ptr_as_ptr)]
#[allow(clippy::cast_ptr_alignment)]
pub fn archived_repr<T: rkyv::Archive>(addr: *const u8) -> &'static <T as rkyv::Archive>::Archived {
    let mem_len = unsafe { *{ addr as *const u32 } } as usize;
    unsafe {
        let mem_slice = &*slice_from_raw_parts::<u8>(addr.add(4), mem_len);
        rkyv::access_unchecked::<T>(mem_slice)
    }
}

/// Get the Program Identifier of the running program, assumes
/// pre-populated memory region starting `_mozak_self_prog_id`.
#[allow(clippy::ptr_as_ptr)]
pub fn get_self_prog_id() -> ProgramIdentifier {
    let self_prog_id = unsafe { *{ addr_of!(_mozak_self_prog_id) as *const ProgramIdentifier } };
    assert!(self_prog_id != ProgramIdentifier::default());
    self_prog_id
}

/// Hashes the input slice to `Poseidon2Hash` after padding.
/// We use the well known "Bit padding scheme".
#[allow(dead_code)]
pub fn poseidon2_hash_with_pad(input: &[u8]) -> Poseidon2Hash {
    let padded_input = mozak_poseidon2::do_padding(input);

    let mut output = [0; DIGEST_BYTES];
    crate::core::ecall::poseidon2(
        padded_input.as_ptr(),
        padded_input.len(),
        output.as_mut_ptr(),
    );
    Poseidon2Hash(output)
}

/// Hashes the input slice to `Poseidon2Hash`, assuming
/// the slice length to be of multiple of `RATE`.
/// # Panics
/// If the slice length is not multiple of `RATE`.
/// This is intentional since zkvm's proof system
/// would fail otherwise.
#[allow(dead_code)]
pub fn poseidon2_hash_no_pad(input: &[u8]) -> Poseidon2Hash {
    assert!(input.len() % mozak_poseidon2::DATA_PADDING == 0);
    let mut output = [0; DIGEST_BYTES];
    crate::core::ecall::poseidon2(input.as_ptr(), input.len(), output.as_mut_ptr());
    Poseidon2Hash(output)
}

/// Given a memory start address with 4-byte length prefix
/// for underlying data, get an owned buffer
macro_rules! get_owned_buffer {
    ($x:expr) => {
        #[allow(clippy::ptr_as_ptr)]
        {
            owned_buffer(unsafe { core::ptr::addr_of!($x) as *const u8 })
        }
    };
}

/// Given a type and the memory start address with 4-byte length prefix
/// for underlying data, get an archived (not fully deserialized) object
macro_rules! get_rkyv_archived {
    ($t:ty, $x:expr) => {
        #[allow(clippy::ptr_as_ptr)]
        {
            archived_repr::<$t>(unsafe { core::ptr::addr_of!($x) as *const u8 })
        }
    };
}

/// Given a type and the memory start address with 4-byte lenght prefix
/// for underlying data, get a fully deserialized object
macro_rules! get_rkyv_deserialized {
    ($t:ty, $x:expr) => {
        #[allow(clippy::ptr_as_ptr)]
        {
            use rkyv::rancor::{Panic, Strategy};
            let archived_repr = get_rkyv_archived!($t, $x);
            let deserialized_repr: $t = archived_repr
                .deserialize(Strategy::<(), Panic>::wrap(&mut ()))
                .unwrap();
            deserialized_repr
        }
    };
}

pub(crate) use {get_owned_buffer, get_rkyv_archived, get_rkyv_deserialized};
