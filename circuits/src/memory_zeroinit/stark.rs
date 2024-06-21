use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::MemoryZeroInit;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryZeroInitConstraints {}

pub type MemoryZeroInitStark<F, const D: usize> =
    StarkFrom<F, MemoryZeroInitConstraints, { D }, COLUMNS, PUBLIC_INPUTS>;

impl<F, const D: usize> HasNamedColumns for MemoryZeroInitStark<F, D> {
    type Columns = MemoryZeroInit<F>;
}

const COLUMNS: usize = MemoryZeroInit::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for MemoryZeroInitConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = MemoryZeroInit<E>;

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
    type S = MemoryZeroInitStark<F, D>;

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
