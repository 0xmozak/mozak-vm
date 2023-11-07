extern crate alloc;

use alloc::vec::Vec;
use core::arch::asm;

static mut OUTPUT_BYTES: Option<Vec<u8>> = None;

#[no_mangle]
pub fn init() {
    unsafe {
        OUTPUT_BYTES = Some(Vec::new());
    }
}

#[no_mangle]
pub fn finalize() {
    // HALT syscall
    //
    // As per RISC-V Calling Convention a0/a1 (which are actually X10/X11) can be
    // used as function argument/result.
    // a0 is used to indicate that its HALT system call.
    // a1 is used to pass output bytes.
    unsafe {
        let output_bytes_vec = OUTPUT_BYTES.as_ref().unwrap_unchecked();
        let output_0 = output_bytes_vec.first().unwrap_unchecked();
        asm!(
            "ecall",
            in ("a0") 0,
            in ("a1") output_0,
        );
    }
    unreachable!();
}

#[no_mangle]
pub fn write(output_data: &[u8]) {
    let output_bytes_vec = unsafe { OUTPUT_BYTES.as_mut().unwrap_unchecked() };
    output_bytes_vec.extend_from_slice(output_data);
}
