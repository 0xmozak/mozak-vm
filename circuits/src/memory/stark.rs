use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use crate::columns_view::NumberOfColumns;
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::memory::columns::Memory;
use crate::unstark::NoColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryConstraints {}

#[allow(clippy::module_name_repetitions)]
pub type MemoryStark<F, const D: usize> =
    StarkFrom<F, MemoryConstraints, { D }, COLUMNS, PUBLIC_INPUTS>;

const COLUMNS: usize = Memory::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for MemoryConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = Memory<E>;

    fn generate_constraints<'a, T: Debug + Copy>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let nv = vars.next_values;
        let mut constraints = ConstraintBuilder::default();

        // Boolean constraints
        // -------------------
        // Constrain certain columns of the memory table to be only
        // boolean values.
        constraints.always(lv.is_writable.is_binary());
        constraints.always(lv.is_store.is_binary());
        constraints.always(lv.is_load.is_binary());
        constraints.always(lv.is_init.is_binary());
        constraints.always(lv.is_executed().is_binary());

        // Address constraints
        // -------------------

        // We start address at 0 and end at u32::MAX
        // This saves rangechecking the addresses
        // themselves, we only rangecheck their difference.
        constraints.first_row(lv.addr - 0);
        constraints.last_row(lv.addr - i64::from(u32::MAX));

        // Address can only change for init in the new row...
        constraints.always((1 - nv.is_init) * (nv.addr - lv.addr));
        // ... and we have a range-check to make sure that addresses go up for each
        // init.

        // Operation constraints
        // ---------------------

        // writeable only changes for init:
        constraints.always((1 - nv.is_init) * (nv.is_writable - lv.is_writable));

        // No `SB` operation can be seen if memory address is not marked `writable`
        constraints.always((1 - lv.is_writable) * lv.is_store);

        // For all "load" operations, the value cannot change between rows
        constraints.always(nv.is_load * (nv.value - lv.value));

        // Padding constraints
        // -------------------
        // Once we have padding, all subsequent rows are padding; ie not
        // `is_executed`.
        constraints.transition((lv.is_executed() - nv.is_executed()) * nv.is_executed());

        constraints
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = MemoryStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_memory_sb_lb_all() -> Result<()> {
        let (program, executed) = memory_trace_test_case(1);
        MozakStark::prove_and_verify(&program, &executed)?;
        Ok(())
    }

    #[test]
    fn prove_memory_sb_lb() -> Result<()> {
        for repeats in 0..8 {
            let (program, executed) = memory_trace_test_case(repeats);
            MemoryStark::prove_and_verify(&program, &executed)?;
        }
        Ok(())
    }

    pub fn memory<Stark: ProveAndVerify>(
        iterations: u32,
        addr_offset: u32,
    ) -> Result<(), anyhow::Error> {
        let instructions = [
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 1,
                    rs1: 1,
                    imm: 1_u32.wrapping_neg(),
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SB,
                args: Args {
                    rs1: 1,
                    rs2: 1,
                    imm: addr_offset,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::BLT,
                args: Args {
                    rs1: 0,
                    rs2: 1,
                    imm: 0,
                    ..Args::default()
                },
            },
        ];
        let (program, record) = code::execute(instructions, &[], &[(1, iterations)]);
        Stark::prove_and_verify(&program, &record)
    }

    #[test]
    fn prove_memory_mozak_example() { memory::<MozakStark<F, D>>(150, 0).unwrap(); }

    use mozak_runner::test_utils::{u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_memory_mozak(iterations in u8_extra(), addr_offset in u32_extra()) {
            memory::<MozakStark<F, D>>(iterations.into(), addr_offset).unwrap();
        }
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
