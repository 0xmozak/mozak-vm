use core::fmt::Debug;
use std::fmt::Display;

use super::columns::RangeCheckColumnsView;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct RangeCheckConstraints {}

const COLUMNS: usize = RangeCheckColumnsView::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for RangeCheckConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = RangeCheckColumnsView<E>;
}

impl Display for RangeCheckConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[allow(clippy::module_name_repetitions)]
pub type RangeCheckStark<F, const D: usize> =
    StarkFrom<F, RangeCheckConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::test_stark_circuit_constraints;

    use super::*;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;
    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RangeCheckStark<F, D>;

    #[test]
    fn test_rangecheck_stark_big_trace() {
        let inst = 1;

        let u16max = u32::from(u16::MAX);
        let mem = (0..=u16max)
            .step_by(23)
            .map(|i| (i, inst))
            .collect::<Vec<_>>();
        let (program, record) = code::execute(
            [Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &mem,
            &[(6, 100), (7, 100)],
        );
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
