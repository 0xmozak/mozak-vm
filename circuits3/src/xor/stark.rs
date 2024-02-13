use itertools::{chain, izip};
use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_matrix::MatrixRowSlices;

use super::columns::XorColumnsView;
use crate::columns_view::NumberOfColumns;
use crate::utils::reduce_with_powers;

struct XorStark;

impl<F> BaseAir<F> for XorStark {
    fn width(&self) -> usize { XorColumnsView::<F>::NUMBER_OF_COLUMNS }
}

impl<AB: AirBuilder> Air<AB> for XorStark {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let lv: &XorColumnsView<AB::Var> = main.row_slice(0).into();

        // Check: bit representation of inputs and output contains either 0 or 1.
        for bit_value in chain!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            builder.assert_bool(bit_value);
        }

        // Check: bit representation of inputs and output were generated correctly.
        for (opx, opx_limbs) in izip![lv.execution, lv.limbs] {
            builder.assert_zero(reduce_with_powers(opx_limbs, &AB::Expr::two()) - opx);
        }

        // Check: output bit representation is Xor of input a and b bit representations
        for (a, b, res) in izip!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            // Note that if a, b are in {0, 1}: (a ^ b) = a + b - 2 * a * b
            // One can check by substituting the values, that:
            //      if a = b = 0            -> 0 + 0 - 2 * 0 * 0 = 0
            //      if only a = 1 or b = 1  -> 1 + 0 - 2 * 1 * 0 = 1
            //      if a = b = 1            -> 1 + 1 - 2 * 1 * 1 = 0
            let xor = (a + b) - (a * b) * AB::Expr::two();
            builder.assert_zero(res - xor);
        }

        // TODO(Kapil): Current version of plonky3 has a bug: it does not support
        // a degree one stark. So we are adding this stupid constraint for a now.
        builder.assert_zero(
            lv.execution.a * lv.is_execution_row * lv.execution.a
                - lv.execution.a * lv.execution.a * lv.is_execution_row,
        );
    }
}

#[cfg(test)]
mod tests {
    use p3_uni_stark::{prove, verify, VerificationError};

    use super::XorStark;
    use crate::config::{BabyBearConfig, DefaultConfig, Mozak3StarkConfig};
    use crate::generation::xor::generate_dummy_xor_trace;

    #[test]
    #[allow(clippy::items_after_statements)]
    fn test_stark() -> Result<(), VerificationError> {
        let n = 12;
        let (config, mut challenger) = DefaultConfig::make_config();
        let mut verifer_challenger = challenger.clone();
        let trace = generate_dummy_xor_trace(n);
        let proof = prove::<<DefaultConfig as Mozak3StarkConfig>::MyConfig, _>(
            &config,
            &XorStark,
            &mut challenger,
            trace,
        );

        verify(&config, &XorStark, &mut verifer_challenger, &proof)
    }

    #[test]
    #[allow(clippy::items_after_statements)]
    fn test_baby_bear_stark() -> Result<(), VerificationError> {
        let n = 12;
        let (config, mut challenger) = BabyBearConfig::make_config();
        let mut verifer_challenger = challenger.clone();
        let trace = generate_dummy_xor_trace(n);
        let proof = prove::<<BabyBearConfig as Mozak3StarkConfig>::MyConfig, _>(
            &config,
            &XorStark,
            &mut challenger,
            trace,
        );

        verify(&config, &XorStark, &mut verifer_challenger, &proof)
    }
}
