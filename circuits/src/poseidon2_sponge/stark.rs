use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
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
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Copy, Clone, Default, StarkNameDisplay)]
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

    // For design check https://docs.google.com/presentation/d/10Dv00xL3uggWTPc0L91cgu_dWUzhM7l1EQ5uDEI_cjg/edit?usp=sharing
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        // NOTE: clk and address will be used for CTL to CPU for is_init_permute rows
        // only, and not be used for permute rows.
        // For all non dummy rows we have CTL to Poseidon2 permute stark, with preimage
        // and output columns.

        let rate = u8::try_from(Poseidon2Permutation::<F>::RATE).expect("rate > 255");
        let state_size = u8::try_from(Poseidon2Permutation::<F>::WIDTH).expect("state_size > 255");
        let rate_scalar = P::Scalar::from_canonical_u8(rate);
        let lv: &Poseidon2Sponge<P> = vars.get_local_values().into();
        let nv: &Poseidon2Sponge<P> = vars.get_next_values().into();

        for val in [lv.ops.is_permute, lv.ops.is_init_permute, lv.gen_output] {
            is_binary(yield_constr, val);
        }
        let is_exe = lv.ops.is_init_permute + lv.ops.is_permute;
        is_binary(yield_constr, is_exe);

        let is_dummy = P::ONES - is_exe;

        // dummy row does not generate output
        yield_constr.constraint(is_dummy * lv.gen_output);

        // if row generates output then it must be last rate sized
        // chunk of input.
        yield_constr.constraint(lv.gen_output * (lv.input_len - rate_scalar));

        let is_init_or_dummy = |vars: &Poseidon2Sponge<P>| {
            (P::ONES - vars.ops.is_init_permute) * (vars.ops.is_init_permute + vars.ops.is_permute)
        };

        // First row must be init permute or dummy row.
        yield_constr.constraint_first_row(is_init_or_dummy(lv));
        // if row generates output then next row can be dummy or start of next hashing
        yield_constr.constraint(lv.gen_output * is_init_or_dummy(nv));

        // Clk should not change within a sponge
        yield_constr.constraint_transition(nv.ops.is_permute * (lv.clk - nv.clk));

        let not_last_sponge =
            (P::ONES - lv.gen_output) * (lv.ops.is_permute + lv.ops.is_init_permute);
        // if current row consumes input and its not last sponge then next row must have
        // length decreases by RATE, note that only actual execution row can consume
        // input
        yield_constr
            .constraint_transition(not_last_sponge * (lv.input_len - (nv.input_len + rate_scalar)));
        // and input_addr increases by RATE
        yield_constr.constraint_transition(
            not_last_sponge * (lv.input_addr - (nv.input_addr - rate_scalar)),
        );

        // For each init_permute capacity bits are zero.
        (rate..state_size).for_each(|i| {
            yield_constr.constraint(lv.ops.is_init_permute * (lv.preimage[i as usize] - P::ZEROS));
        });

        // For each permute capacity bits are copied from previous output.
        (rate..state_size).for_each(|i| {
            yield_constr.constraint(
                (P::ONES - nv.ops.is_init_permute)
                    * nv.ops.is_permute
                    * (nv.preimage[i as usize] - lv.output[i as usize]),
            );
        });
    }

    #[allow(clippy::similar_names)]
    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &Poseidon2Sponge<ExtensionTarget<D>> = vars.get_local_values().into();
        let nv: &Poseidon2Sponge<ExtensionTarget<D>> = vars.get_next_values().into();

        let rate = u8::try_from(Poseidon2Permutation::<F>::RATE).expect("rate > 255");
        let state_size = u8::try_from(Poseidon2Permutation::<F>::WIDTH).expect("state_size > 255");

        for val in [lv.ops.is_permute, lv.ops.is_init_permute, lv.gen_output] {
            is_binary_ext_circuit(builder, val, yield_constr);
        }
        let is_exe = builder.add_extension(lv.ops.is_init_permute, lv.ops.is_permute);
        is_binary_ext_circuit(builder, is_exe, yield_constr);

        let one = builder.constant_extension(F::Extension::from_canonical_u8(1));
        let is_dummy = builder.sub_extension(one, is_exe);

        // dummy row does not generate output
        let dummy_mul_get_output = builder.mul_extension(is_dummy, lv.gen_output);
        yield_constr.constraint(builder, dummy_mul_get_output);

        // if row generates output then it must be last rate sized
        // chunk of input.
        let rate_ext = builder.constant_extension(F::Extension::from_canonical_u8(rate));
        let input_len_sub_rate = builder.sub_extension(lv.input_len, rate_ext);
        let gen_op_len_check = builder.mul_extension(lv.gen_output, input_len_sub_rate);
        yield_constr.constraint(builder, gen_op_len_check);

        // First row must be init permute or dummy row.
        let is_init_lv = builder.sub_extension(one, lv.ops.is_init_permute);
        let is_dummy_lv = builder.add_extension(lv.ops.is_init_permute, lv.ops.is_permute);
        let is_init_or_is_dummy_lv = builder.mul_extension(is_init_lv, is_dummy_lv);
        yield_constr.constraint_first_row(builder, is_init_or_is_dummy_lv);

        // if row generates output then next row can be dummy or start of next hashing
        let is_init_nv = builder.sub_extension(one, nv.ops.is_init_permute);
        let is_dummy_nv = builder.add_extension(nv.ops.is_init_permute, nv.ops.is_permute);
        let is_init_or_is_dummy_nv = builder.mul_extension(is_init_nv, is_dummy_nv);
        let gen_op_nv_check = builder.mul_extension(lv.gen_output, is_init_or_is_dummy_nv);
        yield_constr.constraint(builder, gen_op_nv_check);

        // Clk should not change within a sponge
        let clk_diff = builder.sub_extension(lv.clk, nv.clk);
        let clk_check = builder.mul_extension(nv.ops.is_permute, clk_diff);
        yield_constr.constraint_transition(builder, clk_check);

        let is_dummy_lv = builder.add_extension(lv.ops.is_init_permute, lv.ops.is_permute);
        let not_gen_op = builder.sub_extension(one, lv.gen_output);
        let not_last_sponge = builder.mul_extension(not_gen_op, is_dummy_lv);

        let nv_input_len_rate = builder.add_extension(nv.input_len, rate_ext);
        let input_len_diff_rate = builder.sub_extension(lv.input_len, nv_input_len_rate);
        let len_check = builder.mul_extension(not_last_sponge, input_len_diff_rate);
        // if current row consumes input and its not last sponge then next row must have
        // length decreases by RATE, note that only actual execution row can consume
        // input
        yield_constr.constraint_transition(builder, len_check);
        // and input_addr increases by RATE
        let nv_input_addr_rate = builder.sub_extension(nv.input_addr, rate_ext);
        let input_addr_diff_rate = builder.sub_extension(lv.input_addr, nv_input_addr_rate);
        let addr_check = builder.mul_extension(not_last_sponge, input_addr_diff_rate);
        yield_constr.constraint_transition(builder, addr_check);

        let zero = builder.constant_extension(F::Extension::ZERO);
        // For each init_permute capacity bits are zero.
        (rate..state_size).for_each(|i| {
            let value_sub_zero = builder.sub_extension(lv.preimage[i as usize], zero);
            let zero_check = builder.mul_extension(lv.ops.is_init_permute, value_sub_zero);
            yield_constr.constraint(builder, zero_check);
        });

        // For each permute capacity bits are copied from previous output.
        (rate..state_size).for_each(|i| {
            let is_not_init_perm = builder.sub_extension(one, nv.ops.is_init_permute);
            let is_perm_mul_not_init_perm =
                builder.mul_extension(is_not_init_perm, nv.ops.is_permute);
            let value_sub_output =
                builder.sub_extension(nv.preimage[i as usize], lv.output[i as usize]);
            let value_check = builder.mul_extension(is_perm_mul_not_init_perm, value_sub_output);
            yield_constr.constraint(builder, value_check);
        });
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::prover::prove;
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use super::Poseidon2SpongeStark;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2SpongeStark<F, D>;

    fn poseidon2_sponge_constraints(tests: &[Poseidon2Test]) -> Result<()> {
        let _ = env_logger::try_init();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;
        config.fri_config.rate_bits = 3; // to meet the constraint degree bound

        let (_program, record) = create_poseidon2_test(tests);

        let step_rows = record.executed;

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
    fn prove_poseidon2_sponge() {
        assert!(poseidon2_sponge_constraints(&[Poseidon2Test {
            data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }])
        .is_ok());
    }
    #[test]
    fn prove_poseidon2_sponge_multiple() {
        assert!(poseidon2_sponge_constraints(&[
            Poseidon2Test {
                data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
                input_start_addr: 512,
                output_start_addr: 1024,
            },
            Poseidon2Test {
                data: "😇 Mozak is knowledge arguments based technology".to_string(),
                input_start_addr: 1024 + 32, /* make sure input and output do not overlap with
                                              * earlier call */
                output_start_addr: 2048,
            },
        ])
        .is_ok());
    }

    #[test]
    fn poseidon2_stark_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }
    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
