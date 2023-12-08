use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_dft::Radix2Bowers;
use p3_field::extension::BinomialExtensionField;
use p3_field::Field;
use p3_fri::{FriBasedPcs, FriConfigImpl, FriLdt};
use p3_goldilocks::Goldilocks;
use p3_keccak::Keccak256Hash;
use p3_ldt::QuotientMmcs;
use p3_mds::integrated_coset_mds::IntegratedCosetMds;
use p3_merkle_tree::FieldMerkleTreeMmcs;
use p3_poseidon2::{DiffusionMatrixGoldilocks, Poseidon2};
use p3_symmetric::{SerializingHasher64, TruncatedPermutation};
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
    type Quotient;
    type MyFriConfig;
    type MyConfig;

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
    /// Function used to combine `2` (hashed) nodes of Merkle tree
    type MyCompress = TruncatedPermutation<Self::Perm, 2, { Self::CHUNK }, { Self::WIDTH }>;
    type MyConfig = StarkConfigImpl<
        Self::Val,
        Self::Challenge,
        Self::PackedChallenge,
        Self::Pcs,
        Self::Challenger,
    >;
    type MyFriConfig = FriConfigImpl<
        Self::Val,
        Self::Challenge,
        Self::Quotient,
        Self::ChallengeMmcs,
        Self::Challenger,
    >;
    type MyHash = SerializingHasher64<Keccak256Hash>;
    type MyMds = IntegratedCosetMds<Self::Val, { Self::WIDTH }>;
    type PackedChallenge = BinomialExtensionField<<Self::Val as Field>::Packing, { Self::D }>;
    type Pcs = FriBasedPcs<Self::MyFriConfig, Self::ValMmcs, Self::Dft, Self::Challenger>;
    /// Poseidon2 with sbox degree 7 (Since 7 is smallest prime dividing (p-1))
    type Perm = Poseidon2<Self::Val, Self::MyMds, DiffusionMatrixGoldilocks, { Self::WIDTH }, 7>;
    type Quotient = QuotientMmcs<Self::Val, Self::Challenge, Self::ValMmcs>;
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
        let fri_config = Self::MyFriConfig::new(40, challenge_mmcs);
        let ldt = FriLdt { config: fri_config };
        let pcs = Self::Pcs::new(dft, val_mmcs, ldt);
        (
            Self::MyConfig::new(pcs),
            Self::Challenger::new(perm.clone()),
        )
    }
}
