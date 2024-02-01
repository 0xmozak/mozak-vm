use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_matrix::MatrixRowSlices;

use super::columns::BitShift;
use crate::columns_view::NumberOfColumns;

pub struct BitShiftStark;

impl<F> BaseAir<F> for BitShiftStark {
    fn width(&self) -> usize { BitShift::<F>::NUMBER_OF_COLUMNS }
}

impl<AB: AirBuilder> Air<AB> for BitShiftStark {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let blah = main.row_slice(0);
        dbg!(blah.len());
        let local: &BitShift<AB::Var> = main.row_slice(0).into();
        let next: &BitShift<AB::Var> = main.row_slice(1).into();

        // NOTE: currently, plonky3 doesn't have API to calculate
        // degree of constraint. Till it is fixed in upstream, it
        // can be assumed to have been hardcoded to 3 for now.

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

        // TODO(Kapil): Current version of plonky3 has a bug: it does not support
        // a degree one stark. So we are adding this stupid constraint for a now.
        builder.assert_zero(
            local.amount * local.multiplier * local.amount
                - local.amount * local.amount * local.multiplier,
        );
    }
}

#[cfg(test)]
mod tests {
    use p3_uni_stark::{prove, verify, VerificationError};

    use crate::bitshift::stark::BitShiftStark;
    use crate::config::{DefaultConfig, Mozak3StarkConfig};
    use crate::generation::bitshift::generate_bitshift_trace;

    #[test]
    #[allow(clippy::items_after_statements)]
    fn test_stark() -> Result<(), VerificationError> {
        let (config, mut challenger) = DefaultConfig::make_config();
        let mut verifer_challenger = challenger.clone();
        let trace = generate_bitshift_trace();
        let proof = prove::<<DefaultConfig as Mozak3StarkConfig>::MyConfig, _>(
            &config,
            &BitShiftStark,
            &mut challenger,
            trace,
        );

        verify(&config, &BitShiftStark, &mut verifer_challenger, &proof)
    }
}
