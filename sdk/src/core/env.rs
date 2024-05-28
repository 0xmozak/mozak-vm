#[cfg(target_os = "mozakvm")]
extern crate alloc;

#[cfg(all(target_os = "mozakvm", not(feature = "std")))]
use alloc::vec::Vec;

#[cfg(target_os = "mozakvm")]
static mut OUTPUT_BYTES: Vec<u8> = Vec::new();

#[cfg(target_os = "mozakvm")]
pub fn init() {}

#[cfg(target_os = "mozakvm")]
pub fn finalize() {
    unsafe {
        super::ecall::halt(OUTPUT_BYTES.first().unwrap_or_default());
    }
}

#[allow(dead_code)]
pub fn write(output_data: &[u8]) {
    #[cfg(target_os = "mozakvm")]
    OUTPUT_BYTES.extend_from_slice(output_data);

    #[cfg(not(target_os = "mozakvm"))]
    core::hint::black_box(output_data);
}
