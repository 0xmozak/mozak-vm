use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_matrix::MatrixRowSlices;

use super::columns::Add;
use crate::columns_view::NumberOfColumns;

struct AddStark;

impl<F> BaseAir<F> for AddStark {
    fn width(&self) -> usize { Add::<F>::NUMBER_OF_COLUMNS }
}

impl<AB: AirBuilder> Air<AB> for AddStark {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local: &Add<AB::Var> = main.row_slice(0).into();

        let one = AB::F::one();
        let base = AB::F::from_canonical_u32(1 << 8);

        let carry_1 = local.carry[0];
        let carry_2 = local.carry[1];
        let carry_3 = local.carry[2];

        let overflow_0 = local.op1[0] + local.op2[0] - local.out[0];
        let overflow_1 = local.op1[1] + local.op2[1] - local.out[1] + carry_1;
        let overflow_2 = local.op1[2] + local.op2[2] - local.out[2] + carry_2;
        let overflow_3 = local.op1[3] + local.op2[3] - local.out[3] + carry_3;

        builder.assert_zero(overflow_0.clone() * (overflow_0.clone() - base));
        builder.assert_zero(overflow_1.clone() * (overflow_1.clone() - base));
        builder.assert_zero(overflow_2.clone() * (overflow_2.clone() - base));
        builder.assert_zero(overflow_3.clone() * (overflow_3 - base));

        builder.assert_zero(overflow_0.clone() * (carry_1 - one) + (overflow_0 - base) * carry_1);
        builder.assert_zero(overflow_1.clone() * (carry_2 - one) + (overflow_1 - base) * carry_2);
        builder.assert_zero(overflow_2.clone() * (carry_3 - one) + (overflow_2 - base) * carry_3);
        builder.assert_bool(carry_1);
        builder.assert_bool(carry_2);
        builder.assert_bool(carry_3);
    }
}

#[cfg(test)]
mod tests {
    use mozak_runner::test_utils::u32_extra;
    use p3_uni_stark::{prove, verify};
    use proptest::proptest;

    use super::AddStark;
    use crate::config::{BabyBearConfig, Mozak3StarkConfig};
    use crate::generation::cpu::generate_add_trace;
    proptest! {
        #[test]
        fn test_add_baby_bear_stark(op1 in u32_extra(), op2 in u32_extra()) {
        let (config, mut challenger) = BabyBearConfig::make_config();
        let mut verifer_challenger = challenger.clone();
        let trace = generate_add_trace(op1, op2, op1.wrapping_add(op2));
        let proof = prove::<<BabyBearConfig as Mozak3StarkConfig>::MyConfig, _>(
            &config,
            &AddStark,
            &mut challenger,
            trace,
        );

        let res = verify(&config, &AddStark, &mut verifer_challenger, &proof);
        assert!(res.is_ok());
    }
    }
}
