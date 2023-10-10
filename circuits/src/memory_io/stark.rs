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

use crate::memory_io::columns::{InputOutputMemory, NUM_HW_MEM_COLS};
use crate::stark::utils::is_binary;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct InputOuputMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> Display for InputOuputMemoryStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InputOutputMemoryStark")
    }
}

const COLUMNS: usize = NUM_HW_MEM_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for InputOuputMemoryStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

        where
            FE: FieldExtension<D2, BaseField = F>,
            P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    // Design description - https://docs.google.com/presentation/d/1J0BJd49BMQh3UR5TrOhe3k67plHxnohFtFVrMpDJ1oc/edit?usp=sharing
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &InputOutputMemory<P> = vars.get_local_values().try_into().unwrap();
        let nv: &InputOutputMemory<P> = vars.get_next_values().try_into().unwrap();

        is_binary(yield_constr, lv.ops.is_memory_store);
        is_binary(yield_constr, lv.ops.is_memory_load);
        is_binary(yield_constr, lv.ops.is_io_store);
        is_binary(yield_constr, lv.ops.is_io_load);
        is_binary(yield_constr, lv.is_executed());

        // If nv.is_io() == 1: lv.size == 0
        yield_constr.constraint_transition(nv.is_io() * lv.size);
        // If nv.is_memory() == 1:
        //    lv.address == nv.address + 1 (wrapped)
        //    lv.size == nv.size - 1 (not-wrapped)
        let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
        let added = nv.address + P::ONES;
        let wrapped = added - wrap_at;
        yield_constr
            .constraint_transition(nv.is_memory() * (lv.address - added) * (lv.address - wrapped));
        yield_constr.constraint_transition(nv.is_io() * (lv.size - (nv.size - P::ONES)));
    }

    #[coverage(off)]
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
