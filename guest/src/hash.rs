pub const DIGEST_BYTES: usize = 32;
pub const RATE: usize = 8;

pub struct Digest([u8; DIGEST_BYTES]);

impl Digest {
    pub const fn new(data: [u8; DIGEST_BYTES]) -> Self { Self(data) }

    pub fn as_bytes(&self) -> &[u8] { &self.0 }
}

pub fn poseidon2_hash(input: &[u8]) -> Digest {
    #[cfg(target_os = "zkvm")]
    {
        use mozak_system::system::syscall_poseidon2;
        // VM expects input length to be multiple of RATE
        assert!(input.len() % RATE == 0);
        let mut output = [0; DIGEST_BYTES];
        syscall_poseidon2(input.as_ptr(), input.len(), output.as_mut_ptr());
        Digest::new(output)
    }
    #[cfg(not(target_os = "zkvm"))]
    {
        use plonky2::field::goldilocks_field::GoldilocksField;
        use plonky2::field::types::Field;
        use plonky2::hash::poseidon2::Poseidon2Hash;
        use plonky2::plonk::config::{GenericHashOut, Hasher};
        let data_fields: Vec<GoldilocksField> = input
            .iter()
            .map(|x| GoldilocksField::from_canonical_u8(*x))
            .collect();
        Digest::new(
            Poseidon2Hash::hash_no_pad(&data_fields)
                .to_bytes()
                .try_into()
                .expect("can't fail"),
        )
    }
}
