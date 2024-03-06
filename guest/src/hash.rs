pub const DIGEST_BYTES: usize = 32;
pub const RATE: usize = 8;

#[derive(PartialEq, Eq, Debug)]
pub struct Digest([u8; DIGEST_BYTES]);

impl core::ops::Deref for Digest {
    type Target = [u8; DIGEST_BYTES];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Digest {
    pub const fn new(data: [u8; DIGEST_BYTES]) -> Self { Self(data) }

    pub fn as_bytes(&self) -> &[u8] { &self.0 }
}

pub fn poseidon2_hash(input: &[u8]) -> Digest {
    #[cfg(target_os = "mozakvm")]
    {
        use mozak_system::system::syscall_poseidon2;
        // VM expects input length to be multiple of RATE
        assert!(input.len() % RATE == 0);
        let mut output = [0; DIGEST_BYTES];
        syscall_poseidon2(input.as_ptr(), input.len(), output.as_mut_ptr());
        Digest::new(output)
    }
    #[cfg(not(target_os = "mozakvm"))]
    {
        use plonky2::field::goldilocks_field::GoldilocksField;
        use plonky2::field::types::Field;
        use plonky2::hash::poseidon2::Poseidon2Hash;
        use plonky2::plonk::config::{GenericHashOut, Hasher};
        let data_fields: rust_alloc::vec::Vec<GoldilocksField> = input
            .iter()
            .map(|x| GoldilocksField::from_canonical_u8(*x))
            .collect();
        Digest::new(
            Poseidon2Hash::hash_no_pad(&data_fields)
                .to_bytes()
                .try_into()
                .expect("Output length does not match to DIGEST_BYTES"),
        )
    }
}
