#[cfg(target_os = "mozakvm")]
use core::arch::asm;

#[cfg(target_os = "mozakvm")]
use crate::core::constants::DIGEST_BYTES;

pub const HALT: u32 = 0;
pub const PANIC: u32 = 1;
pub const PRIVATE_TAPE: u32 = 2;
pub const POSEIDON2: u32 = 3;
pub const PUBLIC_TAPE: u32 = 4;
pub const CALL_TAPE: u32 = 5;
pub const EVENT_TAPE: u32 = 6;
pub const EVENTS_COMMITMENT_TAPE: u32 = 7;
pub const CAST_LIST_COMMITMENT_TAPE: u32 = 8;
pub const SELF_PROG_ID_TAPE: u32 = 9;
/// Syscall to output the VM trace log at `clk`. Useful for debugging.
pub const VM_TRACE_LOG: u32 = 10;

#[must_use]
pub fn log<'a>(raw_id: u32) -> &'a str {
    match raw_id {
        HALT => "halt",
        PANIC => "panic",
        PUBLIC_TAPE => "ioread public tape",
        POSEIDON2 => "poseidon2",
        PRIVATE_TAPE => "ioread private tape",
        CALL_TAPE => "ioread call tape",
        EVENT_TAPE => "ioread event tape",
        EVENTS_COMMITMENT_TAPE => "ioread events commitment tape",
        CAST_LIST_COMMITMENT_TAPE => "ioread cast list commitment tape",
        SELF_PROG_ID_TAPE => "self prog id tape",
        VM_TRACE_LOG => "vm trace log",
        _ => "",
    }
}

#[cfg(target_os = "mozakvm")]
pub fn poseidon2(input_ptr: *const u8, input_len: usize, output_ptr: *mut u8) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") POSEIDON2,
            in ("a1") input_ptr,
            in ("a2") input_len,
            in ("a3") output_ptr,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn ioread_private(buf: &mut [u8]) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") PRIVATE_TAPE,
            in ("a1") buf.as_mut_ptr(),
            in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn ioread_public(buf: &mut [u8]) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") PUBLIC_TAPE,
        in ("a1") buf.as_mut_ptr(),
        in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn call_tape_read(buf: &mut [u8]) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") CALL_TAPE,
        in ("a1") buf.as_mut_ptr(),
        in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn event_tape_read(buf: &mut [u8]) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") EVENT_TAPE,
        in ("a1") buf.as_mut_ptr(),
        in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn events_commitment_tape_read(buf: &mut [u8]) {
    assert!(buf.len() == DIGEST_BYTES);
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") EVENTS_COMMITMENT_TAPE,
        in ("a1") buf.as_mut_ptr(),
        in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn cast_list_commitment_tape_read(buf: &mut [u8]) {
    assert!(buf.len() == DIGEST_BYTES);
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") CAST_LIST_COMMITMENT_TAPE,
        in ("a1") buf.as_mut_ptr(),
        in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn self_prog_id_tape_read(buf: &mut [u8]) {
    assert!(buf.len() == DIGEST_BYTES);
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") SELF_PROG_ID_TAPE,
        in ("a1") buf.as_mut_ptr(),
        in ("a2") buf.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn panic(msg: &str) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") PANIC,
            in ("a1") msg.as_ptr(),
            in ("a2") msg.len(),
        );
    }
}

#[cfg(all(target_os = "mozakvm", feature = "trace"))]
pub fn trace(msg: &str) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") VM_TRACE_LOG,
            in ("a1") msg.as_ptr(),
            in ("a2") msg.len(),
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn halt(output: u8) {
    unsafe {
        asm!(
            "ecall",
            in ("a0") HALT,
            in ("a1") output,
        );
        unreachable!();
    }
}
