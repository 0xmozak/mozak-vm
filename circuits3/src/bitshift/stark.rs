use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_matrix::MatrixRowSlices;

use super::columns::BitShift;

struct BitShiftStark;

impl<F> BaseAir<F> for BitShiftStark {}

impl<AB: AirBuilder> Air<AB> for BitShiftStark {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local: &BitShift<AB::Var> = main.row_slice(0).into();
        let next: &BitShift<AB::Var> = main.row_slice(1).into();

        // first amount value is 0
        builder.when_first_row().assert_zero(local.amount);

        // amount always increases by one
        builder
            .when_transition()
            .assert_one(next.amount - local.amount);

        // amount last row value is 31
        builder
            .when_last_row()
            .assert_eq(local.amount, AB::Expr::from_canonical_u32(31));

        // first multiplier value is 1
        builder.when_first_row().assert_one(local.multiplier);

        // multiplier always gets multiplied by 2
        builder
            .when_transition()
            .assert_zero(local.multiplier + local.multiplier - next.multiplier);

        // last multiplier value is 2^31
        builder
            .when_last_row()
            .assert_eq(local.multiplier, AB::Expr::from_canonical_u32(1 << 31));
    }
}

#[cfg(test)]
mod tests {
    use p3_baby_bear::BabyBear;
    use p3_challenger::DuplexChallenger;
    use p3_commit::ExtensionMmcs;
    use p3_dft::{Radix2Bowers, Radix2DitParallel};
    use p3_field::extension::BinomialExtensionField;
    use p3_field::Field;
    use p3_fri::{FriBasedPcs, FriConfigImpl, FriLdt};
    use p3_keccak::Keccak256Hash;
    use p3_ldt::QuotientMmcs;
    use p3_mds::coset_mds::CosetMds;
    use p3_merkle_tree::FieldMerkleTreeMmcs;
    use p3_poseidon::Poseidon;
    use p3_symmetric::{
        CompressionFunctionFromHasher, PaddingFreeSponge, SerializingHasher32, TruncatedPermutation,
    };
    use p3_uni_stark::{prove, verify, StarkConfigImpl, VerificationError};
    use rand::thread_rng;

    use crate::bitshift::stark::BitShiftStark;
    use crate::generation::bitshift::generate_bitshift_trace;

    #[test]
    fn test_stark() -> Result<(), VerificationError> {
        type Val = BabyBear;
        type Challenge = BinomialExtensionField<Val, 5>;
        type PackedChallenge = BinomialExtensionField<<Val as Field>::Packing, 5>;

        type Mds16 = CosetMds<Val, 16>;
        let mds16 = Mds16::default();

        type Perm16 = Poseidon<Val, Mds16, 16, 5>;
        let perm = Perm16::new_from_rng(4, 22, mds16, &mut thread_rng()); // TODO: Use deterministic RNG

        type MyHash = SerializingHasher32<Keccak256Hash>;
        let hash = MyHash::new(Keccak256Hash {});

        type MyCompress = CompressionFunctionFromHasher<Val, MyHash, 2, 8>;
        let compress = MyCompress::new(hash);

        type ValMmcs = FieldMerkleTreeMmcs<Val, MyHash, MyCompress, 8>;
        let val_mmcs = ValMmcs::new(hash, compress);

        type ChallengeMmcs = ExtensionMmcs<Val, Challenge, ValMmcs>;
        let challenge_mmcs = ChallengeMmcs::new(val_mmcs.clone());

        type Dft = Radix2Bowers;
        let dft = Dft::default();

        type Challenger = DuplexChallenger<Val, Perm16, 16>;

        type Quotient = QuotientMmcs<Val, Challenge, ValMmcs>;
        type MyFriConfig = FriConfigImpl<Val, Challenge, Quotient, ChallengeMmcs, Challenger>;
        let fri_config = MyFriConfig::new(40, challenge_mmcs);
        let ldt = FriLdt { config: fri_config };

        type Pcs = FriBasedPcs<MyFriConfig, ValMmcs, Dft, Challenger>;
        type MyConfig = StarkConfigImpl<Val, Challenge, PackedChallenge, Pcs, Challenger>;

        let pcs = Pcs::new(dft, val_mmcs, ldt);
        let config = MyConfig::new(pcs);
        let mut challenger = Challenger::new(perm.clone());
        let trace = generate_bitshift_trace::<Val>();
        let proof = prove::<MyConfig, _>(&config, &BitShiftStark, &mut challenger, trace);

        let mut challenger = Challenger::new(perm);
        verify(&config, &BitShiftStark, &mut challenger, &proof)
    }
}
