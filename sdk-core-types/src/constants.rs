pub mod poseidon2 {
    /// The size of a `Poseidon2Hash` digest in bytes.
    pub const DIGEST_BYTES: usize = 32;

    /// `RATE` of `Poseidon2Permutation` we use
    #[allow(dead_code)]
    pub const RATE: usize = 8;
}
