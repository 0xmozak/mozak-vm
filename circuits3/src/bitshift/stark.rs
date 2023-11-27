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

        // // amount always increases by one
        builder
            .when_transition()
            .assert_zero(next.amount - local.amount - AB::Expr::one());

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
    use p3_uni_stark::{prove, verify, StarkConfigImpl, VerificationError};
    use rand::thread_rng;

    use crate::bitshift::stark::BitShiftStark;
    use crate::generation::bitshift::generate_bitshift_trace;

    #[test]
    #[allow(clippy::items_after_statements)]
    fn test_stark() -> Result<(), VerificationError> {
        // TODO: figure out what each of these hardcoded values mean
        type Val = Goldilocks;
        type Domain = Val;
        type Challenge = BinomialExtensionField<Val, 2>;
        type PackedChallenge = BinomialExtensionField<<Domain as Field>::Packing, 2>;

        type MyMds = CosetMds<Val, 16>;
        let mds = MyMds::default();

        type Perm = Poseidon2<Val, MyMds, DiffusionMatrixGoldilocks, 16, 5>;
        let perm = Perm::new_from_rng(8, 22, mds, DiffusionMatrixGoldilocks, &mut thread_rng());

        type MyHash = PaddingFreeSponge<Perm, 16, 8, 8>;
        let hash = MyHash::new(perm.clone());

        type MyCompress = TruncatedPermutation<Perm, 2, 8, 16>;
        let compress = MyCompress::new(perm.clone());

        type ValMmcs = FieldMerkleTreeMmcs<<Val as Field>::Packing, MyHash, MyCompress, 8>;
        let val_mmcs = ValMmcs::new(hash, compress);

        type ChallengeMmcs = ExtensionMmcs<Val, Challenge, ValMmcs>;
        let challenge_mmcs = ChallengeMmcs::new(val_mmcs.clone());

        type Dft = Radix2DitParallel;
        let dft = Dft {};

        type Challenger = DuplexChallenger<Val, Perm, 16>;

        type Quotient = QuotientMmcs<Domain, Challenge, ValMmcs>;
        type MyFriConfig = FriConfigImpl<Val, Challenge, Quotient, ChallengeMmcs, Challenger>;
        let fri_config = MyFriConfig::new(40, challenge_mmcs);
        let ldt = FriLdt { config: fri_config };

        type Pcs = FriBasedPcs<MyFriConfig, ValMmcs, Dft, Challenger>;
        type MyConfig = StarkConfigImpl<Val, Challenge, PackedChallenge, Pcs, Challenger>;

        let pcs = Pcs::new(dft, val_mmcs, ldt);
        let config = StarkConfigImpl::new(pcs);
        let mut challenger = Challenger::new(perm.clone());
        let trace = generate_bitshift_trace();
        let proof = prove::<MyConfig, _>(&config, &BitShiftStark, &mut challenger, trace);

        let mut challenger = Challenger::new(perm);
        verify(&config, &BitShiftStark, &mut challenger, &proof)
    }
}
