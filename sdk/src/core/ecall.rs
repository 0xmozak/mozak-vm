#[cfg(target_os = "mozakvm")]
use core::arch::asm;

pub const HALT: u32 = 0;
pub const PANIC: u32 = 1;
pub const IO_READ_PRIVATE: u32 = 2;
pub const POSEIDON2: u32 = 3;
pub const IO_READ_PUBLIC: u32 = 4;
pub const IO_READ_TRANSCRIPT: u32 = 5;
/// Syscall to output the VM trace log at `clk`. Useful for debugging.
pub const VM_TRACE_LOG: u32 = 6;

#[must_use]
pub fn log<'a>(raw_id: u32) -> &'a str {
    match raw_id {
        HALT => "halt",
        PANIC => "panic",
        IO_READ_PUBLIC => "ioread public tape",
        POSEIDON2 => "poseidon2",
        IO_READ_PRIVATE => "ioread private tape",
        IO_READ_TRANSCRIPT => "ioread transcript",
        VM_TRACE_LOG => "vm trace log",
        _ => "",
    }
}

#[allow(dead_code)]
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
        let _ = input_ptr;
        let _ = input_len;
        let _ = output_ptr;
        unimplemented!()
    }
}

#[allow(dead_code)]
pub fn ioread_private(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(all(target_os = "mozakvm", not(feature = "mozak-ro-memory")))]
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") IO_READ_PRIVATE,
            in ("a1") buf_ptr,
            in ("a2") buf_len,
        );
    }
    #[cfg(all(target_os = "mozakvm", feature = "mozak-ro-memory"))]
    // TODO(Roman): later on please add assert(capacity >= buf_len)
    // NOTE: it is up to the application owner how to implement this, it can be implemented using
    // zero-copy later on we will change our default implementation to be zero-copy: `buf_ptr =
    // _mozak_private_io_tape`
    unsafe {
        extern "C" {
            #[link_name = "_mozak_private_io_tape"]
            static _mozak_private_io_tape: usize;
        }
        let io_tape_ptr = &raw const _mozak_private_io_tape as *const u8;
        for i in 0..isize::try_from(buf_len)
            .expect("ecall_ioread_private: usize to isize cast should succeed for buf_len")
        {
            buf_ptr.offset(i).write(io_tape_ptr.offset(i).read());
        }
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        let _ = buf_ptr;
        let _ = buf_len;
        unimplemented!()
    }
}

#[allow(dead_code)]
pub fn ioread_public(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(all(target_os = "mozakvm", not(feature = "mozak-ro-memory")))]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") IO_READ_PUBLIC,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
    #[cfg(all(target_os = "mozakvm", feature = "mozak-ro-memory"))]
    // TODO(Roman): later on please add assert(capacity >= buf_len)
    // NOTE: it is up to the application owner how to implement this, it can be implemented using
    // zero-copy later on we will change our default implementation to be zero-copy: `buf_ptr =
    // _mozak_public_io_tape`
    unsafe {
        extern "C" {
            #[link_name = "_mozak_public_io_tape"]
            static _mozak_public_io_tape: usize;
        }
        let io_tape_ptr = &raw const _mozak_public_io_tape as *const u8;
        for i in 0..isize::try_from(buf_len)
            .expect("ecall_ioread_public: usize to isize cast should succeed for buf_len")
        {
            buf_ptr.offset(i).write(io_tape_ptr.offset(i).read());
        }
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        let _ = buf_ptr;
        let _ = buf_len;
        unimplemented!()
    }
}

#[allow(dead_code)]
pub fn transcript_read(buf_ptr: *mut u8, buf_len: usize) {
    #[cfg(all(target_os = "mozakvm", not(feature = "mozak-ro-memory")))]
    unsafe {
        core::arch::asm!(
        "ecall",
        in ("a0") IO_READ_TRANSCRIPT,
        in ("a1") buf_ptr,
        in ("a2") buf_len,
        );
    }
    #[cfg(all(target_os = "mozakvm", feature = "mozak-ro-memory"))]
    // TODO(Roman): later on please add assert(capacity >= buf_len)
    // NOTE: it is up to the application owner how to implement this, it can be implemented using
    // zero-copy later on we will change our default implementation to be zero-copy: `buf_ptr =
    // _mozak_transcript`
    unsafe {
        extern "C" {
            #[link_name = "_mozak_transcript"]
            static _mozak_transcript: usize;
        }
        let io_tape_ptr = &raw const _mozak_transcript as *const u8;
        for i in 0..isize::try_from(buf_len)
            .expect("ecall_transcript_read: usize to isize cast should succeed for buf_len")
        {
            buf_ptr.offset(i).write(io_tape_ptr.offset(i).read());
        }
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        let _ = buf_ptr;
        let _ = buf_len;
        unimplemented!()
    }
}

#[allow(dead_code)]
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
        let _ = msg_ptr;
        let _ = msg_len;
        unimplemented!()
    }
}

#[allow(dead_code)]
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
        let _ = msg_ptr;
        let _ = msg_len;
        unimplemented!()
    }
}

#[allow(dead_code)]
pub fn halt(output: u8) {
    #[cfg(target_os = "mozakvm")]
    // HALT ecall
    //
    // As per RISC-V Calling Convention a0/a1 (which are actually X10/X11) can be
    // used as function argument/result.
    // a0 is used to indicate that its HALT system call.
    // a1 is used to pass output bytes.
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
        let _ = output;
        unimplemented!()
    }
}
