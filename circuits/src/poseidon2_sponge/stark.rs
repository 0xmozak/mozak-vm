use std::fmt::Display;
use std::marker::PhantomData;

use mozak_runner::poseidon2::NUM_HASH_OUT_ELTS;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::Poseidon2Permutation;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::NUM_POSEIDON2_SPONGE_COLS;
use crate::columns_view::HasNamedColumns;
use crate::display::derive_display_stark_name;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::utils::is_binary;

derive_display_stark_name!(Poseidon2SpongeStark);
#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2SpongeStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for Poseidon2SpongeStark<F, D> {
    type Columns = Poseidon2Sponge<F>;
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

        let rate = u8::try_from(Poseidon2Permutation::<F>::RATE).expect("rate > 255");
        let state_size = u8::try_from(Poseidon2Permutation::<F>::WIDTH).expect("state_size > 255");
        let rate_scalar = P::Scalar::from_canonical_u8(rate);
        let lv: &Poseidon2Sponge<P> = vars.get_local_values().try_into().unwrap();
        let nv: &Poseidon2Sponge<P> = vars.get_next_values().try_into().unwrap();

        is_binary(yield_constr, lv.ops.is_init_permute);
        is_binary(yield_constr, lv.ops.is_permute);
        is_binary(yield_constr, lv.ops.is_init_permute + lv.ops.is_permute);
        is_binary(yield_constr, lv.gen_output);
        is_binary(yield_constr, lv.con_input);

        let is_dummy =
            |vars: &Poseidon2Sponge<P>| P::ONES - (vars.ops.is_init_permute + vars.ops.is_permute);
        is_binary(yield_constr, is_dummy(lv));

        // dummy row does not consume input
        yield_constr.constraint(is_dummy(lv) * lv.con_input);
        // dummy row does not generate output
        yield_constr.constraint(is_dummy(lv) * lv.gen_output);

        // Two consequtive rows can not be is_init_permute. As even for smallest input
        // size (RATE) it needs more than one squeeze rounds so next row can not
        // be init permute.
        yield_constr.constraint(lv.ops.is_init_permute * nv.ops.is_init_permute);

        // if row generates output and consumes input then it must be last rate sized
        // chunk of input.
        yield_constr.constraint(lv.gen_output * lv.con_input * (lv.input_len - rate_scalar));

        // if row generates output and does not consume input then input_len must be
        // zero.
        yield_constr.constraint(lv.gen_output * (P::ONES - lv.con_input) * lv.input_len);

        let num_hash_out_elements_scalar = P::Scalar::from_canonical_u8(
            u8::try_from(NUM_HASH_OUT_ELTS).expect("num hash output > 255"),
        );
        // if row generates output and next row is dummy then it must be last RATE size
        // chunk of output.
        yield_constr.constraint(
            lv.gen_output
                * is_dummy(nv)
                * (lv.output_len - (num_hash_out_elements_scalar - rate_scalar)),
        );

        // if row generates output and next row is init_permute then it must be last
        // RATE size chunk of output
        yield_constr.constraint(
            lv.gen_output
                * nv.ops.is_init_permute
                * (lv.output_len - (num_hash_out_elements_scalar - rate_scalar)),
        );

        // if current row consumes input then next row must have
        // length decreases by RATE, note that only actaul execution row can consume
        // input
        yield_constr
            .constraint_transition(lv.con_input * (lv.input_len - (nv.input_len + rate_scalar)));
        // and input_addr increases by RATE
        yield_constr
            .constraint_transition(lv.con_input * (lv.input_addr - (nv.input_addr - rate_scalar)));

        // if current row generates output then next row mst have output_addr increased
        // by RATE
        yield_constr.constraint_transition(
            lv.gen_output * (lv.output_addr - (nv.output_addr - rate_scalar)),
        );
        // and output lenght is increased by RATE in next row.
        yield_constr
            .constraint_transition(lv.gen_output * (nv.output_len - (lv.output_len + rate_scalar)));

        // For each init_permute capacity bits are zero.
        (rate..state_size).for_each(|i| {
            yield_constr.constraint(lv.ops.is_init_permute * (lv.preimage[i as usize] - P::ZEROS));
        });

        // For each permute capacity bits are copied from previous output.
        (rate..state_size).for_each(|i| {
            yield_constr
                .constraint(nv.ops.is_permute * (nv.preimage[i as usize] - lv.output[i as usize]));
        });

        // For each permute if input is not consumed then rate bits are copied from
        // previous output.
        (0..rate).for_each(|i| {
            yield_constr.constraint(
                nv.ops.is_permute
                    * (P::ONES - nv.con_input)
                    * (nv.preimage[i as usize] - lv.output[i as usize]),
            );
        });
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
    use mozak_runner::poseidon2::hash_n_to_m_with_pad;
    use mozak_runner::state::{Aux, Poseidon2Entry};
    use mozak_runner::vm::Row;
    use plonky2::field::types::Sample;
    use plonky2::hash::hashing::PlonkyPermutation;
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
        let _ = env_logger::try_init();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;
        config.fri_config.rate_bits = 3; // to meet the constraint degree bound

        let mut step_rows = vec![];
        let mut input = vec![];
        // VM expects input lenght to be multiple of RATE bits
        let input_len = input_len
            .next_multiple_of(u32::try_from(Poseidon2Permutation::<F>::RATE).expect("RATE > 2^32"));
        for _ in 0..input_len {
            input.push(F::rand());
        }
        let (_hash, sponge_data) =
            hash_n_to_m_with_pad::<F, Poseidon2Permutation<F>>(input.as_slice());
        step_rows.push(Row {
            aux: Aux {
                poseidon2: Some(Poseidon2Entry::<F> {
                    sponge_data,
                    len: input_len,
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
