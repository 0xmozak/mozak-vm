#[cfg(target_os = "mozakvm")]
extern crate alloc;

#[cfg(target_os = "mozakvm")]
static mut OUTPUT_BYTES: Option<Vec<u8>> = None;

#[cfg(target_os = "mozakvm")]
pub fn init() {
    unsafe {
        OUTPUT_BYTES = Some(Vec::new());
    }
}

#[cfg(target_os = "mozakvm")]
pub fn finalize() {
    unsafe {
        super::ecall::halt(
            OUTPUT_BYTES
                .as_ref()
                .and_then(|v| v.first().copied())
                .unwrap_or_default(),
        );
    }
}

#[allow(dead_code)]
pub fn write(output_data: &[u8]) {
    #[cfg(target_os = "mozakvm")]
    {
        let output_bytes_vec = unsafe { OUTPUT_BYTES.as_mut().unwrap_unchecked() };
        output_bytes_vec.extend_from_slice(output_data);
    }
    #[cfg(not(target_os = "mozakvm"))]
    core::hint::black_box(output_data);
}
