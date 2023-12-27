use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::reduce_with_powers;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::{FIELDS_COUNT, NUM_POSEIDON2_OUTPUT_BYTES_COLS};
use crate::columns_view::HasNamedColumns;
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytes;

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
        let lv: &Poseidon2OutputBytes<_> = vars.get_local_values().into();
        for i in 0..FIELDS_COUNT {
            let start_index = i * 8;
            let end_index = i * 8 + 8;
            yield_constr.constraint(
                reduce_with_powers(
                    &lv.output_bytes[start_index..end_index],
                    P::Scalar::from_canonical_u16(256),
                ) - lv.output_fields[i],
            );
        }
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
    use mozak_runner::test_utils::u64_extra;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, PrimeField64};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::plonk::plonk_common::reduce_with_powers;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::ProptestConfig;
    use proptest::{prop_assert_eq, proptest};
    use starky::config::StarkConfig;
    use starky::prover::prove;
    use starky::stark_testing::test_stark_low_degree;
    use starky::verifier::verify_stark_proof;

    use super::Poseidon2OutputBytesStark;
    use crate::generation::poseidon2_output_bytes::generate_poseidon2_output_bytes_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
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
}
