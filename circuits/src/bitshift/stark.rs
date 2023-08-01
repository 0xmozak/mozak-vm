use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::ShiftAmountView;
use crate::columns_view::NumberOfColumns;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct ShiftAmountStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for ShiftAmountStark<F, D> {
    const COLUMNS: usize = ShiftAmountView::<()>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &ShiftAmountView<P> = vars.local_values.borrow();
        let nv: &ShiftAmountView<P> = vars.next_values.borrow();

        // Constraints on shift amount
        yield_constr.constraint_first_row(lv.executed.amount);
        yield_constr.constraint_transition(
            (nv.executed.amount - lv.executed.amount - P::ONES)
                * (nv.executed.amount - lv.executed.amount),
        );
        yield_constr.constraint_last_row(lv.executed.amount - P::Scalar::from_canonical_u8(31));

        // Constraints on multiplier
        let diff = nv.executed.amount - lv.executed.amount;
        yield_constr.constraint_first_row(lv.executed.multiplier - P::ONES);
        yield_constr.constraint_transition(
            nv.executed.multiplier - (P::ONES + diff) * lv.executed.multiplier,
        );
        yield_constr
            .constraint_last_row(lv.executed.multiplier - P::Scalar::from_canonical_u32(1 << 31));
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
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use proptest::{prop_assert_eq, proptest};
    use starky::stark_testing::test_stark_low_degree;

    use super::ShiftAmountStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = ShiftAmountStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    proptest! {
        #[test]
        fn prove_shift_amount_proptest(p in u32_extra(), q in u32_extra()) {
            let record = simple_test_code(
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
            ShiftAmountStark::prove_and_verify(&record.executed).unwrap();
        }
    }
}
