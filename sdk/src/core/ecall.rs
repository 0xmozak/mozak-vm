#![cfg_attr(not(target_os = "mozakvm"), allow(unused_variables))]
#[cfg(target_os = "mozakvm")]
use core::arch::asm;

pub const HALT: u32 = 0;
pub const PANIC: u32 = 1;
pub const IO_READ_PRIVATE: u32 = 2;
pub const POSEIDON2: u32 = 3;
pub const IO_READ_PUBLIC: u32 = 4;
pub const IO_READ_CALL_TAPE: u32 = 5;
pub const EVENT_TAPE: u32 = 6;
pub const EVENTS_COMMITMENT_TAPE: u32 = 7;
pub const CAST_LIST_COMMITMENT_TAPE: u32 = 8;
/// Syscall to output the VM trace log at `clk`. Useful for debugging.
pub const VM_TRACE_LOG: u32 = 9;

/// The number of bytes requested for events commitment tape and
/// cast list commitment tape is hardcoded to 32 bytes.
pub const COMMITMENT_SIZE: usize = 32;

#[must_use]
pub fn log<'a>(raw_id: u32) -> &'a str {
    match raw_id {
        HALT => "halt",
        PANIC => "panic",
        IO_READ_PUBLIC => "ioread public tape",
        POSEIDON2 => "poseidon2",
        IO_READ_PRIVATE => "ioread private tape",
        IO_READ_CALL_TAPE => "ioread call tape",
        EVENT_TAPE => "ioread event tape",
        EVENTS_COMMITMENT_TAPE => "ioread events commitment tape",
        CAST_LIST_COMMITMENT_TAPE => "ioread cast list commitment tape",
        VM_TRACE_LOG => "vm trace log",
        _ => "",
    }
}

pub fn poseidon2(input_ptr: *const u8, input_len: usize, output_ptr: *mut u8) {
    #[cfg(target_os = "mozakvm")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") POSEIDON2,
            in ("a1") input_ptr,
            in ("a2") input_len,
            in ("a3") output_ptr,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn ioread_private(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(target_os = "mozakvm")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") IO_READ_PRIVATE,
            in ("a1") buf_ptr,
            in ("a2") buf_len,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn ioread_public(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(target_os = "mozakvm")]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") IO_READ_PUBLIC,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn call_tape_read(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(all(target_os = "mozakvm"))]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") IO_READ_CALL_TAPE,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn events_tape_read(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(all(target_os = "mozakvm"))]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") EVENT_TAPE,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn events_commitment_tape_read(buf_ptr: *mut u8) {
    #[cfg(all(target_os = "mozakvm", not(feature = "mozak-ro-memory")))]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") EVENTS_COMMITMENT_TAPE,
        in ("a1") buf_ptr,
        in ("a2") COMMITMENT_SIZE,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn cast_list_commitment_tape_read(buf_ptr: *mut u8) {
    #[cfg(all(target_os = "mozakvm", not(feature = "mozak-ro-memory")))]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") CAST_LIST_COMMITMENT_TAPE,
        in ("a1") buf_ptr,
        in ("a2") COMMITMENT_SIZE,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn panic(msg_ptr: *const u8, msg_len: usize) {
    #[cfg(target_os = "mozakvm")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") PANIC,
            in ("a1") msg_len,
            in ("a2") msg_ptr,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn trace(msg_ptr: *const u8, msg_len: usize) {
    #[cfg(target_os = "mozakvm")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") VM_TRACE_LOG,
            in ("a1") msg_len,
            in ("a2") msg_ptr,
        );
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}

pub fn halt(output: u8) {
    #[cfg(target_os = "mozakvm")]
    unsafe {
        asm!(
            "ecall",
            in ("a0") HALT,
            in ("a1") output,
        );
        unreachable!();
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        unimplemented!()
    }
}
