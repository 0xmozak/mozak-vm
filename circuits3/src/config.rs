use p3_baby_bear::BabyBear;
use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_dft::Radix2Bowers;
use p3_field::extension::BinomialExtensionField;
use p3_field::Field;
use p3_fri::{FriConfig, TwoAdicFriPcs, TwoAdicFriPcsConfig};
use p3_goldilocks::Goldilocks;
use p3_keccak::Keccak256Hash;
use p3_mds::integrated_coset_mds::IntegratedCosetMds;
use p3_merkle_tree::FieldMerkleTreeMmcs;
use p3_poseidon2::{DiffusionMatrixBabybear, DiffusionMatrixGoldilocks, Poseidon2};
use p3_symmetric::{SerializingHasher32, SerializingHasher64, TruncatedPermutation};
use p3_uni_stark::StarkConfigImpl;
use rand::thread_rng;

/// This config refers to types required to use Plonky3 `uni_stark` prover
/// and verifier
#[allow(clippy::pedantic)]
pub trait Mozak3StarkConfig {
    /// Degree of extension field the challenge lies in
    const D: usize;
    /// Max size of array on which Permutation can operate
    const WIDTH: usize;
    /// Number of field elements required to represent output of MyHash
    const CHUNK: usize;
    type Val;
    type Challenge;
    type PackedChallenge;
    type Pcs;
    type Challenger;
    type MyMds;
    type Perm;
    type MyHash;
    type MyCompress;
    type ValMmcs;
    type ChallengeMmcs;
    type Dft;
    type MyConfig;
    type FriConfig;

    fn make_config() -> (Self::MyConfig, Self::Challenger);
}

#[allow(clippy::module_name_repetitions)]
pub struct DefaultConfig;

impl Mozak3StarkConfig for DefaultConfig {
    // TODO: Figure out meaning of the hardcoded constants
    type Challenge = BinomialExtensionField<Self::Val, { Self::D }>;
    type ChallengeMmcs = ExtensionMmcs<Self::Val, Self::Challenge, Self::ValMmcs>;
    type Challenger = DuplexChallenger<Self::Val, Self::Perm, { Self::WIDTH }>;
    type Dft = Radix2Bowers;
    type FriConfig = FriConfig<Self::ChallengeMmcs>;
    /// Function used to combine `2` (hashed) nodes of Merkle tree
    type MyCompress = TruncatedPermutation<Self::Perm, 2, { Self::CHUNK }, { Self::WIDTH }>;
    type MyConfig = StarkConfigImpl<
        Self::Val,
        Self::Challenge,
        Self::PackedChallenge,
        Self::Pcs,
        Self::Challenger,
    >;
    type MyHash = SerializingHasher64<Keccak256Hash>;
    type MyMds = IntegratedCosetMds<Self::Val, { Self::WIDTH }>;
    type PackedChallenge = BinomialExtensionField<<Self::Val as Field>::Packing, { Self::D }>;
    type Pcs = TwoAdicFriPcs<
        TwoAdicFriPcsConfig<
            Self::Val,
            Self::Challenge,
            Self::Challenger,
            Self::Dft,
            Self::ValMmcs,
            Self::ChallengeMmcs,
        >,
    >;
    /// Poseidon2 with sbox degree 7 (Since 7 is smallest prime not dividing
    /// (p-1))
    type Perm = Poseidon2<Self::Val, Self::MyMds, DiffusionMatrixGoldilocks, { Self::WIDTH }, 7>;
    type Val = Goldilocks;
    type ValMmcs = FieldMerkleTreeMmcs<
        <Self::Val as Field>::Packing,
        Self::MyHash,
        Self::MyCompress,
        { Self::CHUNK },
    >;

    /// Since `MyHash` outputs 32 bytes, we can use 256/64 = 4 Field elements
    /// to represent it.
    const CHUNK: usize = 4;
    const D: usize = 2;
    const WIDTH: usize = 16;

    fn make_config() -> (Self::MyConfig, Self::Challenger) {
        let mds = Self::MyMds::default();
        let perm =
            Self::Perm::new_from_rng(8, 22, mds, DiffusionMatrixGoldilocks, &mut thread_rng());
        let hash = Self::MyHash::new(Keccak256Hash {});
        let compress = Self::MyCompress::new(perm.clone());
        let val_mmcs = Self::ValMmcs::new(hash, compress);
        let challenge_mmcs = Self::ChallengeMmcs::new(val_mmcs.clone());
        let dft = Self::Dft {};
        let fri_config = Self::FriConfig {
            log_blowup: 1,
            num_queries: 40,
            proof_of_work_bits: 8,
            mmcs: challenge_mmcs,
        };
        let pcs = Self::Pcs::new(fri_config, dft, val_mmcs);
        (
            Self::MyConfig::new(pcs),
            Self::Challenger::new(perm.clone()),
        )
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct BabyBearConfig;

impl Mozak3StarkConfig for BabyBearConfig {
    type Challenge = BinomialExtensionField<Self::Val, { Self::D }>;
    type ChallengeMmcs = ExtensionMmcs<Self::Val, Self::Challenge, Self::ValMmcs>;
    type Challenger = DuplexChallenger<Self::Val, Self::Perm, { Self::WIDTH }>;
    type Dft = Radix2Bowers;
    type FriConfig = FriConfig<Self::ChallengeMmcs>;
    /// Function used to combine `2` (hashed) nodes of Merkle tree
    type MyCompress = TruncatedPermutation<Self::Perm, 2, { Self::CHUNK }, { Self::WIDTH }>;
    type MyConfig = StarkConfigImpl<
        Self::Val,
        Self::Challenge,
        Self::PackedChallenge,
        Self::Pcs,
        Self::Challenger,
    >;
    type MyHash = SerializingHasher32<Keccak256Hash>;
    type MyMds = IntegratedCosetMds<Self::Val, { Self::WIDTH }>;
    type PackedChallenge = BinomialExtensionField<<Self::Val as Field>::Packing, { Self::D }>;
    type Pcs = TwoAdicFriPcs<
        TwoAdicFriPcsConfig<
            Self::Val,
            Self::Challenge,
            Self::Challenger,
            Self::Dft,
            Self::ValMmcs,
            Self::ChallengeMmcs,
        >,
    >;
    /// Poseidon2 with sbox degree 7 (Since 7 is smallest prime not dividing
    /// (p-1))
    type Perm = Poseidon2<Self::Val, Self::MyMds, DiffusionMatrixBabybear, { Self::WIDTH }, 7>;
    type Val = BabyBear;
    type ValMmcs = FieldMerkleTreeMmcs<
        <Self::Val as Field>::Packing,
        Self::MyHash,
        Self::MyCompress,
        { Self::CHUNK },
    >;

    /// Since `MyHash` outputs 32 bytes, we can use 256/32 = 8 Field elements
    /// to represent it.
    const CHUNK: usize = 8;
    const D: usize = 4;
    const WIDTH: usize = 16;

    fn make_config() -> (Self::MyConfig, Self::Challenger) {
        let mds = Self::MyMds::default();
        let perm = Self::Perm::new_from_rng(8, 22, mds, DiffusionMatrixBabybear, &mut thread_rng());
        let hash = Self::MyHash::new(Keccak256Hash {});
        let compress = Self::MyCompress::new(perm.clone());
        let val_mmcs = Self::ValMmcs::new(hash, compress);
        let challenge_mmcs = Self::ChallengeMmcs::new(val_mmcs.clone());
        let dft = Self::Dft {};
        let fri_config = Self::FriConfig {
            log_blowup: 1,
            num_queries: 40,
            proof_of_work_bits: 8,
            mmcs: challenge_mmcs,
        };
        let pcs = Self::Pcs::new(fri_config, dft, val_mmcs);
        (
            Self::MyConfig::new(pcs),
            Self::Challenger::new(perm.clone()),
        )
    }
}
