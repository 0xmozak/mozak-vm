use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_dft::Radix2DitParallel;
use p3_field::extension::BinomialExtensionField;
use p3_field::Field;
use p3_fri::{FriBasedPcs, FriConfigImpl, FriLdt};
use p3_goldilocks::Goldilocks;
use p3_ldt::QuotientMmcs;
use p3_mds::coset_mds::CosetMds;
use p3_merkle_tree::FieldMerkleTreeMmcs;
use p3_poseidon2::{DiffusionMatrixGoldilocks, Poseidon2};
use p3_symmetric::{PaddingFreeSponge, TruncatedPermutation};
use p3_uni_stark::StarkConfigImpl;
use rand::thread_rng;

/// This config refers to types required to use Plonky3 `uni_stark` prover
/// and verifier
pub trait Mozak3StarkConfig {
    type Val;
    type Domain;
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

pub struct DefaultConfig;

impl Mozak3StarkConfig for DefaultConfig {
    // TODO: Figure out meaning of the hardcoded constants
    type Challenge = BinomialExtensionField<Self::Val, 2>;
    type ChallengeMmcs = ExtensionMmcs<Self::Val, Self::Challenge, Self::ValMmcs>;
    type Challenger = DuplexChallenger<Self::Val, Self::Perm, 16>;
    type Dft = Radix2DitParallel;
    type Domain = Self::Val;
    type MyCompress = TruncatedPermutation<Self::Perm, 2, 8, 16>;
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
    type MyHash = PaddingFreeSponge<Self::Perm, 16, 8, 8>;
    type MyMds = CosetMds<Self::Val, 16>;
    type PackedChallenge = BinomialExtensionField<<Self::Domain as Field>::Packing, 2>;
    type Pcs = FriBasedPcs<Self::MyFriConfig, Self::ValMmcs, Self::Dft, Self::Challenger>;
    type Perm = Poseidon2<Self::Val, Self::MyMds, DiffusionMatrixGoldilocks, 16, 5>;
    type Quotient = QuotientMmcs<Self::Domain, Self::Challenge, Self::ValMmcs>;
    type Val = Goldilocks;
    type ValMmcs =
        FieldMerkleTreeMmcs<<Self::Val as Field>::Packing, Self::MyHash, Self::MyCompress, 8>;

    fn make_config() -> (Self::MyConfig, Self::Challenger) {
        let mds = Self::MyMds::default();
        let perm =
            Self::Perm::new_from_rng(8, 22, mds, DiffusionMatrixGoldilocks, &mut thread_rng());
        let hash = Self::MyHash::new(perm.clone());
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
