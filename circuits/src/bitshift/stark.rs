use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{Bitshift, BitshiftView};
use crate::columns_view::NumberOfColumns;
use crate::stark::mozak_stark::Id;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct BitshiftStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> Id for BitshiftStark<F, D> {
    fn id() -> String {
    "BitshiftStark".to_string()
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for BitshiftStark<F, D> {
    const COLUMNS: usize = BitshiftView::<()>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &BitshiftView<P> = vars.local_values.borrow();
        let nv: &BitshiftView<P> = vars.next_values.borrow();
        let lv: &Bitshift<P> = &lv.executed;
        let nv: &Bitshift<P> = &nv.executed;

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
        yield_constr.constraint_first_row(lv.amount);
        // Check: amount value is increased by 1 or kept unchanged
        yield_constr.constraint_transition(diff * (diff - P::ONES));
        // Check: last amount value is set to 31
        yield_constr.constraint_last_row(lv.amount - P::Scalar::from_canonical_u8(31));

        // Constraints on multiplier
        // They ensure:
        //  1. Shift multiplier is multiplied by 2 only if amount increases.
        //  2. We have shift multiplier from 1 to max possible value of 2^31.

        // Check: initial multiplier value is set to 1 = 2^0
        yield_constr.constraint_first_row(lv.multiplier - P::ONES);
        // Check: multiplier value is doubled if amount is increased
        yield_constr.constraint_transition(nv.multiplier - (P::ONES + diff) * lv.multiplier);
        // Check: last multiplier value is set to 2^31
        // (Note that based on the previous constraint, this is already
        //  satisfied if the last amount value is 31. We leave it for readability.)
        yield_constr.constraint_last_row(lv.multiplier - P::Scalar::from_canonical_u32(1 << 31));
    }

    fn constraint_degree(&self) -> usize { 3 }

    #[no_coverage]
    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{simple_test_code, u32_extra};
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use proptest::{prop_assert_eq, proptest};
    use starky::stark_testing::test_stark_low_degree;

    use super::BitshiftStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
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
        let (program, record) = simple_test_code(&[sll, sll, sll], &[], &[(7, p), (8, q)]);
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
        let (program, record) = simple_test_code(&[srl, srl, srl], &[], &[(7, p), (8, q)]);
        assert_eq!(record.executed[0].aux.dst_val, p >> (q & 0x1F));
        MozakStark::prove_and_verify(&program, &record)
    }

    proptest! {
        #[test]
        fn prove_shift_amount_proptest(p in u32_extra(), q in u32_extra()) {
            let (program, record) = simple_test_code(
                &[Instruction {
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
}
