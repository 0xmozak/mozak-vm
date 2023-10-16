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

use super::columns::NUM_POSEIDON2_SPONGE_COLS;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::utils::is_binary;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2SpongeStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> Display for Poseidon2SpongeStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Poseidon2SpongeStark")
    }
}

const COLUMNS: usize = NUM_POSEIDON2_SPONGE_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for Poseidon2SpongeStark<F, D> {
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
        // Questions: clk and address will be used for CTL for is_init_permut rows only,
        // and not be used for permute rows. Should we add constraints for them here?

        let lv: &Poseidon2Sponge<P> = vars.get_local_values().try_into().unwrap();
        let nv: &Poseidon2Sponge<P> = vars.get_next_values().try_into().unwrap();

        is_binary(yield_constr, lv.ops.is_init_permute);
        is_binary(yield_constr, lv.ops.is_permute);
        is_binary(yield_constr, lv.is_exe);

        // dummy row as is_permute = 0 and is_init_permute = 0
        yield_constr.constraint((lv.is_exe - P::ONES) * lv.ops.is_permute * lv.ops.is_init_permute);

        // if current row is not dummy and next row is not is_init_permute
        // start_index decreases by 8
        yield_constr.constraint(
            lv.is_exe
                * (nv.ops.is_init_permute - P::ONES)
                * (lv.start_index - (nv.start_index + P::Scalar::from_canonical_u8(8))),
        );

        // For each init_permute capacity bits are zero.
        yield_constr.constraint(lv.ops.is_init_permute * (lv.preimage[8] - P::ZEROS));
        yield_constr.constraint(lv.ops.is_init_permute * (lv.preimage[9] - P::ZEROS));
        yield_constr.constraint(lv.ops.is_init_permute * (lv.preimage[10] - P::ZEROS));
        yield_constr.constraint(lv.ops.is_init_permute * (lv.preimage[11] - P::ZEROS));

        // For each permute capacity bits are copied from previous output.
        yield_constr.constraint(nv.ops.is_permute * (nv.preimage[8] - lv.output[8]));
        yield_constr.constraint(nv.ops.is_permute * (nv.preimage[9] - lv.output[9]));
        yield_constr.constraint(nv.ops.is_permute * (nv.preimage[10] - lv.output[10]));
        yield_constr.constraint(nv.ops.is_permute * (nv.preimage[11] - lv.output[11]));
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
