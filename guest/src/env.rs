#[cfg(target_os = "zkvm")]
extern crate alloc;

#[cfg(target_os = "zkvm")]
use alloc::vec::Vec;

#[cfg(target_os = "zkvm")]
static mut OUTPUT_BYTES: Option<Vec<u8>> = None;

#[no_mangle]
pub fn init() {
    #[cfg(target_os = "zkvm")]
    unsafe {
        OUTPUT_BYTES = Some(Vec::new());
    }
}

#[no_mangle]
pub fn finalize() {
    #[cfg(target_os = "zkvm")]
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
    #[cfg(not(target_os = "zkvm"))]
    core::hint::black_box(output_data);
}
