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

use super::columns::ProgramMult;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct ProgramMultStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for ProgramMultStark<F, D> {
    type Columns = ProgramMult<F>;
}

const COLUMNS: usize = ProgramMult::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 1;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for ProgramMultStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &ProgramMult<P> = vars.get_local_values().into();
        // Any instruction used in CPU should also be in the program
        yield_constr.constraint(lv.mult_in_cpu * (P::ONES - lv.mult_in_rom));
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &ProgramMult<_> = vars.get_local_values().into();
        // Any instruction used in CPU should also be in the program
        let one = builder.one_extension();
        let sub = builder.sub_extension(one, lv.mult_in_rom);
        let mul = builder.mul_extension(lv.mult_in_cpu, sub);
        yield_constr.constraint(builder, mul);
    }

    fn constraint_degree(&self) -> usize { 3 }
}
