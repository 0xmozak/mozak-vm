use std::fmt::Display;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::ProgramRom;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::display::derive_display_stark_name;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

derive_display_stark_name!(ProgramStark);
#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct ProgramStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for ProgramStark<F, D> {
    type Columns = ProgramRom<F>;
}

const COLUMNS: usize = ProgramRom::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for ProgramStark<F, D> {
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
        let lv: &ProgramRom<P> = vars.get_local_values().try_into().unwrap();
        is_binary(yield_constr, lv.filter);
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &ProgramRom<ExtensionTarget<D>> = vars.get_local_values().try_into().unwrap();
        is_binary_ext_circuit(builder, lv.filter, yield_constr);
    }

    fn constraint_degree(&self) -> usize { 3 }
}
