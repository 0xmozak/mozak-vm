use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::{self, RangeCheckColumnsView};
use crate::columns_view::HasNamedColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RangeCheckStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for RangeCheckStark<F, D> {
    type Columns = RangeCheckColumnsView<F>;
}

const COLUMNS: usize = columns::NUM_RC_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RangeCheckStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    // NOTE: Actual range check happens in RangeCheckLimbStark. A CrossTableLookup
    // between RangeCheckStark and others like MemoryStark and CpuStark ensure
    // that both have same value. A CrossTableLookup between RangeCheckStark and
    // RangeCheckLimbStark ensures that each limb from this stark are covered
    // in RangeCheckLimbStark.
    // Here we check if limbs are formed properly
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &RangeCheckColumnsView<P> = vars.get_local_values().into();

        let reduced_limb: P = (0..4)
            .map(|limb| lv.limbs[limb] * FE::from_canonical_u32(1 << (8 * limb)))
            .sum();

        let value = lv.multiplicity_view.value;
        yield_constr.constraint(reduced_limb - value);
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;

    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    #[test]
    fn test_rangecheck_stark_big_trace() {
        let inst = 1;

        let u16max = u32::from(u16::MAX);
        let mem = (0..=u16max)
            .step_by(23)
            .map(|i| (i, inst))
            .collect::<Vec<_>>();
        let (program, record) = simple_test_code(
            &[Instruction {
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
}
