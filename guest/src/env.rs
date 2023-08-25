use core::arch::asm;
use std::vec::Vec;

static mut OUTPUT_BYTES: Option<Vec<u8>> = None;

#[no_mangle]
pub fn init() {
    unsafe {
        OUTPUT_BYTES = Some(Vec::new());
    }
}

#[no_mangle]
pub fn finalize() {
    // Exit syscall
    //
    // As per RISC-V Calling Convention a0/a1 (which are actually X10/X11) can be
    // used as function argument/result.
    unsafe {
        let output_bytes_vec = OUTPUT_BYTES.as_ref().unwrap_unchecked();
        let output_0 = output_bytes_vec.get(0).unwrap_unchecked();
        asm!(
            "add a0, zero, {a0}",
            "li a7, 93",
            "ecall",
            a0 = in(reg) output_0,
        );
    }
    loop {}
}

#[no_mangle]
pub fn write(output_data: &[u8]) {
    unsafe {
        let output_bytes_vec = OUTPUT_BYTES.as_mut().unwrap_unchecked();
        output_bytes_vec.extend_from_slice(output_data);
    }
}
