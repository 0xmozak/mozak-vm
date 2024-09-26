use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::MemoryInit;
use crate::columns_view::NumberOfColumns;
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryInitConstraints {}

#[allow(clippy::module_name_repetitions)]
pub type MemoryInitStark<F, const D: usize> =
    StarkFrom<F, MemoryInitConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

const COLUMNS: usize = MemoryInit::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for MemoryInitConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = MemoryInit<E>;

    fn generate_constraints<'a, T: Debug + Copy>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let mut constraints = ConstraintBuilder::default();

        constraints.always(lv.filter.is_binary());

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
    type S = MemoryInitStark<F, D>;

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
