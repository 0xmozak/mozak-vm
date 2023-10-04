pub const DIGEST_BYTES: usize = 32;

pub struct Digest([u8; DIGEST_BYTES]);

impl Digest {
    pub const fn new(data: [u8; DIGEST_BYTES]) -> Self { Self(data) }

    pub fn as_bytes(&self) -> &[u8] { &self.0 }
}

pub fn poseidon_hash(input: &[u8]) -> Digest {
    let mut output = [0; DIGEST_BYTES];
    #[cfg(target_os = "zkvm")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in ("a0") 3,
            in ("a1") input.as_ptr(),
            in ("a2") input.len(),
            in ("a3") output.as_mut_ptr(),
        );
    }
    Digest::new(output)
}
