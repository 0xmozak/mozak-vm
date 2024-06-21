use core::fmt::Debug;

use expr::{Expr, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::BitshiftView;
use crate::columns_view::{HasNamedColumns_, NumberOfColumns};
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

/// Bitshift Trace Constraints
#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct BitshiftConstraints {}

pub type BitshiftStark<F, const D: usize> =
    StarkFrom<F, BitshiftConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

impl HasNamedColumns_ for BitshiftConstraints {
    type Columns<F> = BitshiftView<F>;
}

const COLUMNS: usize = BitshiftView::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for BitshiftConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = BitshiftView<E>;

    fn generate_constraints<'a, T: Debug + Copy>(
        self,
        vars: &StarkFrameTyped<BitshiftView<Expr<'a, T>>, NoColumns<Expr<'a, T>>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values.executed;
        let nv = vars.next_values.executed;
        let mut constraints = ConstraintBuilder::default();

        // Constraints on shift amount
        // They ensure:
        //  1. Shift amount increases with each row by 0 or 1.
        // (We allow increases of 0 in order to allow the table to add
        //  multiple same value rows. This is needed when we have multiple
        //  `SHL` or `SHR` operations with the same shift amount.)
        //  2. We have shift amounts starting from 0 to max possible value of 31.
        // (This is due to RISC-V max shift amount being 31.)

        let diff = nv.amount - lv.amount;
        // Check: initial amount value is set to 0
        constraints.first_row(lv.amount);
        // Check: amount value is increased by 1 or kept unchanged
        constraints.transition(diff * (diff - 1));
        // Check: last amount value is set to 31
        constraints.last_row(lv.amount - 31);

        // Constraints on multiplier
        // They ensure:
        //  1. Shift multiplier is multiplied by 2 only if amount increases.
        //  2. We have shift multiplier from 1 to max possible value of 2^31.

        // Check: initial multiplier value is set to 1 = 2^0
        constraints.first_row(lv.multiplier - 1);
        // Check: multiplier value is doubled if amount is increased
        constraints.transition(nv.multiplier - (1 + diff) * lv.multiplier);
        // Check: last multiplier value is set to 2^31
        // (Note that based on the previous constraint, this is already
        //  satisfied if the last amount value is 31. We leave it for readability.)
        constraints.last_row(lv.multiplier - (1 << 31));

        constraints
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::u32_extra;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use proptest::{prop_assert_eq, proptest};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use super::BitshiftStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = BitshiftStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_sll() -> Result<()> {
        let p: u32 = 10;
        let q: u32 = 10;
        let sll = Instruction {
            op: Op::SLL,
            args: Args {
                rd: 5,
                rs1: 7,
                rs2: 8,
                ..Args::default()
            },
        };
        // We use 3 similar instructions here to ensure duplicates and padding work
        // during trace generation.
        let (program, record) = code::execute([sll, sll, sll], &[], &[(7, p), (8, q)]);
        assert_eq!(record.executed[0].aux.dst_val, p << (q & 0x1F));
        MozakStark::prove_and_verify(&program, &record)
    }

    #[test]
    fn prove_srl() -> Result<()> {
        let p: u32 = 10;
        let q: u32 = 10;
        let srl = Instruction {
            op: Op::SRL,
            args: Args {
                rd: 5,
                rs1: 7,
                rs2: 8,
                ..Args::default()
            },
        };

        // We use 3 similar instructions here to ensure duplicates and padding work
        // during trace generation.
        let (program, record) = code::execute([srl, srl, srl], &[], &[(7, p), (8, q)]);
        assert_eq!(record.executed[0].aux.dst_val, p >> (q & 0x1F));
        MozakStark::prove_and_verify(&program, &record)
    }

    proptest! {
        #[test]
        fn prove_shift_amount_proptest(p in u32_extra(), q in u32_extra()) {
            let (program, record) = code::execute(
                [Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd: 5,
                        rs1: 7,
                        rs2: 8,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRL,
                    args: Args {
                        rd: 6,
                        rs1: 7,
                        imm: q,
                        ..Args::default()
                    },
                }
                ],
                &[],
                &[(7, p), (8, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0x1F));
            prop_assert_eq!(record.executed[1].aux.dst_val, p >> (q & 0x1F));
            BitshiftStark::prove_and_verify(&program, &record).unwrap();
        }
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
