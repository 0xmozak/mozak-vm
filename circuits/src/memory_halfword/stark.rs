use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::memory_halfword::columns::{HalfWordMemory, NUM_HW_MEM_COLS};
use crate::unstark::NoColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct HalfWordMemoryConstraints {}

pub type HalfWordMemoryStark<F, const D: usize> =
    StarkFrom<F, HalfWordMemoryConstraints, { D }, COLUMNS, PUBLIC_INPUTS>;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for HalfWordMemoryConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = HalfWordMemory<E>;

    // Design description - https://docs.google.com/presentation/d/1J0BJd49BMQh3UR5TrOhe3k67plHxnohFtFVrMpDJ1oc/edit?usp=sharing
    fn generate_constraints<'a, T: Debug + Copy>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let mut constraints = ConstraintBuilder::default();

        constraints.always(lv.ops.is_store.is_binary());
        constraints.always(lv.ops.is_load.is_binary());
        constraints.always(lv.is_executed().is_binary());

        let added = lv.addrs[0] + 1;
        let wrapped = added - (1 << 32);

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        constraints.always(lv.is_executed() * (lv.addrs[1] - added) * (lv.addrs[1] - wrapped));

        constraints
    }
}

const COLUMNS: usize = NUM_HW_MEM_COLS;
const PUBLIC_INPUTS: usize = 0;

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{u32_extra, u8_extra};
    use plonky2::plonk::config::Poseidon2GoldilocksConfig;
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    use starky::stark_testing::test_stark_circuit_constraints;

    use crate::memory_halfword::stark::HalfWordMemoryStark;
    // use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};
    pub fn prove_mem_read_write<Stark: ProveAndVerify>(
        offset: u32,
        imm: u32,
        content: u8,
        is_unsigned: bool,
    ) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::SH,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: if is_unsigned { Op::LHU } else { Op::LH },
                    args: Args {
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[
                (imm.wrapping_add(offset), 0),
                (imm.wrapping_add(offset).wrapping_add(1), 0),
                (imm.wrapping_add(offset).wrapping_add(2), 0),
                (imm.wrapping_add(offset).wrapping_add(3), 0),
            ],
            &[(1, content.into()), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]

        #[test]
        fn prove_mem_read_write_mozak(offset in u32_extra(), imm in u32_extra(), content in u8_extra(), is_unsigned: bool) {
            prove_mem_read_write::<MozakStark<F, D>>(offset, imm, content, is_unsigned);
        }
    }
    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        type C = Poseidon2GoldilocksConfig;
        type S = HalfWordMemoryStark<F, D>;
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
