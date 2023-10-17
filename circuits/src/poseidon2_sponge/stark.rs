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
use crate::display::derive_display_stark_name;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::utils::is_binary;

derive_display_stark_name!(Poseidon2SpongeStark);
#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2SpongeStark<F, const D: usize> {
    pub _f: PhantomData<F>,
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::state::{Aux, Poseidon2Entry};
    use mozak_runner::vm::{hash_n_to_m_with_pad, Row};
    use plonky2::field::types::Sample;
    use plonky2::hash::poseidon2::Poseidon2Permutation;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::prover::prove;
    use starky::stark_testing::test_stark_low_degree;
    use starky::verifier::verify_stark_proof;

    use super::Poseidon2SpongeStark;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::stark::utils::trace_rows_to_poly_values;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2SpongeStark<F, D>;

    fn poseidon2_sponge_constraints(input_len: u32) -> Result<()> {
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;
        config.fri_config.rate_bits = 3; // to meet the constraint degree bound

        let mut step_rows = vec![];
        let mut input = vec![];

        for _ in 0..input_len {
            input.push(F::rand());
        }
        let (_hash, sponge_data) =
            hash_n_to_m_with_pad::<F, Poseidon2Permutation<F>>(input.as_slice());
        let padded_len = |input_len: u32| {
            if input_len % 8 == 0 {
                input_len
            } else {
                input_len + (8 - input_len % 8)
            }
        };
        step_rows.push(Row {
            aux: Aux {
                poseidon2: Some(Poseidon2Entry::<F> {
                    sponge_data,
                    len: padded_len(input_len),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        });

        let stark = S::default();
        let trace = generate_poseidon2_sponge_trace(&step_rows);
        let trace_poly_values = trace_rows_to_poly_values(trace);

        let proof = prove::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;
        verify_stark_proof(stark, proof, &config)
    }

    #[test]
    fn test_with_input_len_multiple_of_8() -> Result<()> { poseidon2_sponge_constraints(96) }
    #[test]
    fn test_with_input_len_not_multiple_of_8() -> Result<()> { poseidon2_sponge_constraints(100) }
    #[test]
    fn test_with_input_len_less_than_8() -> Result<()> { poseidon2_sponge_constraints(5) }

    #[test]
    fn poseidon2_stark_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }
}
