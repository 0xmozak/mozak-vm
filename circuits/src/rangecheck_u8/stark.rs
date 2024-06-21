use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::RangeCheckU8;
use crate::columns_view::NumberOfColumns;
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RangeCheckU8Constraints {}

pub type RangeCheckU8Stark<F, const D: usize> =
    StarkFrom<F, RangeCheckU8Constraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

const COLUMNS: usize = RangeCheckU8::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for RangeCheckU8Constraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = RangeCheckU8<E>;

    fn generate_constraints<'a, T: Copy + Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let nv = vars.next_values;
        let mut constraints = ConstraintBuilder::default();

        // Check: the `element`s form a sequence from 0 to 255
        constraints.first_row(lv.value);
        constraints.transition(nv.value - lv.value - 1);
        constraints.last_row(lv.value - i64::from(u8::MAX));

        constraints
    }
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::test_stark_circuit_constraints;

    use super::*;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RangeCheckU8Stark<F, D>;

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
