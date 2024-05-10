#[cfg(target_os = "mozakvm")]
use core::arch::asm;

#[cfg(target_os = "mozakvm")]
use sdk_core_types::constants::poseidon2::DIGEST_BYTES;
#[cfg(target_os = "mozakvm")]
use sdk_core_types::ecall_id;

#[cfg(target_os = "mozakvm")]
pub fn poseidon2(input_ptr: *const u8, input_len: usize, output_ptr: *mut u8) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") ecall_id::POSEIDON2,
            in ("a1") input_ptr,
            in ("a2") input_len,
            in ("a3") output_ptr,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn ioread_private(buf_ptr: *mut u8, buf_len: usize) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") ecall_id::PRIVATE_TAPE,
            in ("a1") buf_ptr,
            in ("a2") buf_len,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn ioread_public(buf_ptr: *mut u8, buf_len: usize) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") ecall_id::PUBLIC_TAPE,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn call_tape_read(buf_ptr: *mut u8, buf_len: usize) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") ecall_id::CALL_TAPE,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn event_tape_read(buf_ptr: *mut u8, buf_len: usize) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") ecall_id::EVENT_TAPE,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn events_commitment_tape_read(buf_ptr: *mut u8) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") ecall_id::EVENTS_COMMITMENT_TAPE,
        in ("a1") buf_ptr,
        in ("a2") DIGEST_BYTES,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn cast_list_commitment_tape_read(buf_ptr: *mut u8) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") ecall_id::CAST_LIST_COMMITMENT_TAPE,
        in ("a1") buf_ptr,
        in ("a2") DIGEST_BYTES,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn self_prog_id_tape_read(buf_ptr: *mut u8) {
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") ecall_id::SELF_PROG_ID_TAPE,
        in ("a1") buf_ptr,
        in ("a2") DIGEST_BYTES,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn panic(msg_ptr: *const u8, msg_len: usize) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") ecall_id::PANIC,
            in ("a1") msg_len,
            in ("a2") msg_ptr,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn trace(msg_ptr: *const u8, msg_len: usize) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") ecall_id::VM_TRACE_LOG,
            in ("a1") msg_len,
            in ("a2") msg_ptr,
        );
    }
}

#[cfg(target_os = "mozakvm")]
pub fn halt(output: u8) {
    unsafe {
        asm!(
            "ecall",
            in ("a0") ecall_id::HALT,
            in ("a1") output,
        );
        unreachable!();
    }
}
