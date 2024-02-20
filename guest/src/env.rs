#[cfg(target_os = "zkvm")]
extern crate alloc;

#[cfg(target_os = "zkvm")]
use alloc::vec::Vec;

#[cfg(target_os = "zkvm")]
static mut OUTPUT_BYTES: Option<Vec<u8>> = None;

#[cfg(target_os = "zkvm")]
pub fn init() {
    unsafe {
        OUTPUT_BYTES = Some(Vec::new());
    }
}

#[cfg(target_os = "zkvm")]
pub fn finalize() {
    unsafe {
        mozak_system::system::syscall_halt(
            OUTPUT_BYTES
                .as_ref()
                .and_then(|v| v.first().cloned())
                .unwrap_or_default(),
        );
    }
}

pub fn write(output_data: &[u8]) {
    #[cfg(target_os = "zkvm")]
    {
        let output_bytes_vec = unsafe { OUTPUT_BYTES.as_mut().unwrap_unchecked() };
        output_bytes_vec.extend_from_slice(output_data);
    }
    #[cfg(not(target_os = "zkvm"))]
    core::hint::black_box(output_data);
}
