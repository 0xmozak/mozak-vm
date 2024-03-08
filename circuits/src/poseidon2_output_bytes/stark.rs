use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::{reduce_with_powers, reduce_with_powers_ext_circuit};
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::{FIELDS_COUNT, NUM_POSEIDON2_OUTPUT_BYTES_COLS};
use crate::columns_view::HasNamedColumns;
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytes;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2OutputBytesStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for Poseidon2OutputBytesStark<F, D> {
    type Columns = Poseidon2OutputBytes<F>;
}

const COLUMNS: usize = NUM_POSEIDON2_OUTPUT_BYTES_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for Poseidon2OutputBytesStark<F, D> {
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
        let two_to_eight = P::Scalar::from_canonical_u16(256);
        let lv: &Poseidon2OutputBytes<_> = vars.get_local_values().into();
        is_binary(yield_constr, lv.is_executed);
        for i in 0..FIELDS_COUNT {
            let start_index = i * 8;
            let end_index = i * 8 + 8;
            yield_constr.constraint(
                reduce_with_powers(&lv.output_bytes[start_index..end_index], two_to_eight)
                    - lv.output_fields[i],
            );
        }

        let u32_max: P = P::Scalar::from_canonical_u32(u32::MAX).into();
        let one = P::ONES;

        (0..4).for_each(|i| {
            let low_limb = reduce_with_powers(&lv.output_bytes[8 * i..8 * i + 4], two_to_eight);
            let high_limb =
                reduce_with_powers(&lv.output_bytes[8 * i + 4..8 * i + 8], two_to_eight);
            let gap_inv = lv.gap_invs[i];
            yield_constr.constraint(((u32_max - high_limb) * gap_inv - one) * low_limb);
        });
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &Poseidon2OutputBytes<ExtensionTarget<D>> = vars.get_local_values().into();
        let two_to_eight = builder.constant(F::from_canonical_u16(256));
        is_binary_ext_circuit(builder, lv.is_executed, yield_constr);
        for i in 0..FIELDS_COUNT {
            let start_index = i * 8;
            let end_index = i * 8 + 8;
            let x = reduce_with_powers_ext_circuit(
                builder,
                &lv.output_bytes[start_index..end_index],
                two_to_eight,
            );
            let x_sub_of = builder.sub_extension(x, lv.output_fields[i]);
            yield_constr.constraint(builder, x_sub_of);
        }

        let u32_max = builder.constant_extension(F::from_canonical_u32(u32::MAX).into());
        let one = builder.constant_extension(F::ONE.into());

        (0..4).for_each(|i| {
            let low_limb = reduce_with_powers_ext_circuit(
                builder,
                &lv.output_bytes[8 * i..8 * i + 4],
                two_to_eight,
            );
            let high_limb = reduce_with_powers_ext_circuit(
                builder,
                &lv.output_bytes[8 * i + 4..8 * i + 8],
                two_to_eight,
            );
            let gap_inv = lv.gap_invs[i];
            let u32_max_sub_high_limb = builder.sub_extension(u32_max, high_limb);
            let u32_max_sub_high_limb_times_gap_inv_minus_one =
                builder.mul_sub_extension(u32_max_sub_high_limb, gap_inv, one);
            let zero =
                builder.mul_extension(u32_max_sub_high_limb_times_gap_inv_minus_one, low_limb);
            yield_constr.constraint(builder, zero);
        });
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::test_utils::u64_extra;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, PrimeField64};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::plonk::plonk_common::reduce_with_powers;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::ProptestConfig;
    use proptest::{prop_assert_eq, proptest};
    use starky::config::StarkConfig;
    use starky::prover::{prove, prove as prove_table};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use super::Poseidon2OutputBytesStark;
    use crate::generation::poseidon2_output_bytes::{
        generate_poseidon2_output_bytes_trace, pad_trace,
    };
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytes;
    use crate::poseidon2_sponge::columns::Poseidon2Sponge;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2OutputBytesStark<F, D>;

    fn poseidon2_output_bytes_constraints(tests: &[Poseidon2Test]) -> Result<()> {
        let _ = env_logger::try_init();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;
        config.fri_config.rate_bits = 3; // to meet the constraint degree bound

        let (_program, record) = create_poseidon2_test(tests);

        let step_rows = record.executed;

        let stark = S::default();
        let trace = generate_poseidon2_sponge_trace(&step_rows);
        let trace = generate_poseidon2_output_bytes_trace(&trace);
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

    proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000_000))]
    #[test]
    fn test_field_to_bytes(value in u64_extra()) {
        let field = GoldilocksField::from_noncanonical_u64(value);
        let bytes = field.to_canonical_u64().to_le_bytes();
        let bytes_fields : Vec<GoldilocksField> = bytes.iter().map(|v| GoldilocksField::from_canonical_u8(*v)).collect();
        let field_recons = reduce_with_powers(&bytes_fields, GoldilocksField::from_canonical_u16(256));
        prop_assert_eq!(field, field_recons);
    }
    }

    #[test]
    fn prove_poseidon2_sponge() {
        assert!(poseidon2_output_bytes_constraints(&[Poseidon2Test {
            data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }])
        .is_ok());
    }
    #[test]
    fn prove_poseidon2_sponge_multiple() {
        assert!(poseidon2_output_bytes_constraints(&[
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

    proptest! {
    /// Poseidon2OutputBytes stark with output bytes corresponding to
    /// non canonical form of hash (with a limb >= goldilocks prime)
    /// should fail
    #[test]
    #[should_panic = "Constraint failed in"]
    fn non_canonical_hash(value in 0..u32::MAX) {
        fn dummy_trace(value: u32) -> Vec<Poseidon2OutputBytes<F>> {
            let output = [F::from_canonical_u32(value); 12];
            let sponge = Poseidon2Sponge::<F> {
                output,
                gen_output: F::ONE,
                ..Default::default()
            };
            let mut malicious_trace: Vec<Poseidon2OutputBytes<F>> = (&sponge).into();
            // add goldilocks prime to first limb
            let u8_max = F::from_canonical_u8(u8::MAX);
            (4..8).for_each(|i| malicious_trace[0].output_bytes[i] += u8_max);
            malicious_trace[0].output_bytes[0] += F::ONE;

            // test that field elements still correspond to malicious bytes
            let two_to_eight = F::from_canonical_u16(256);
            let output_fields = [0, 1, 2, 3].map(|i| {
                reduce_with_powers(
                    &malicious_trace[0].output_bytes[8 * i..8 * i + 8],
                    two_to_eight,
                )
            });
            assert_eq!(output_fields, malicious_trace[0].output_fields);
            pad_trace(malicious_trace)
        }

        let trace = dummy_trace(value);
        let config = StarkConfig::standard_fast_config();
        let stark = S::default();
        let trace_poly_values = trace_rows_to_poly_values(trace);

        let _proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        );
    }}
}
