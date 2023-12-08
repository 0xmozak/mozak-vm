#[cfg(target_os = "zkvm")]
extern crate alloc;

#[cfg(target_os = "zkvm")]
use alloc::vec::Vec;

#[cfg(target_os = "zkvm")]
static mut OUTPUT_BYTES: Option<Vec<u8>> = None;

#[no_mangle]
#[cfg(target_os = "zkvm")]
pub fn init() {
    unsafe {
        OUTPUT_BYTES = Some(Vec::new());
    }
}

#[no_mangle]
#[cfg(target_os = "zkvm")]
pub fn finalize() {
    unsafe {
        let output_bytes_vec = OUTPUT_BYTES.as_ref().unwrap_unchecked();
        let output_0 = output_bytes_vec.first().unwrap_unchecked();
        mozak_system::system::syscall_halt(*output_0);
    }
}

#[no_mangle]
pub fn write(output_data: &[u8]) {
#[cfg(target_os = "zkvm")]
    {
    let output_bytes_vec = unsafe { OUTPUT_BYTES.as_mut().unwrap_unchecked() };
    output_bytes_vec.extend_from_slice(output_data);
    }
}
