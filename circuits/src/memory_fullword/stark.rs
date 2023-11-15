use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use crate::columns_view::HasNamedColumns;
use crate::memory_fullword::columns::{FullWordMemory, NUM_HW_MEM_COLS};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct FullWordMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for FullWordMemoryStark<F, D> {
    type Columns = FullWordMemory<F>;
}

const COLUMNS: usize = NUM_HW_MEM_COLS;
const PUBLIC_INPUTS: usize = 0;
impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for FullWordMemoryStark<F, D> {
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
        let lv: &FullWordMemory<P> = vars.get_local_values().into();

        is_binary(yield_constr, lv.ops.is_store);
        is_binary(yield_constr, lv.ops.is_load);
        is_binary(yield_constr, lv.is_executed());

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
        for i in 1..4 {
            let target = lv.addrs[0] + P::Scalar::from_canonical_usize(i);
            yield_constr.constraint(
                lv.is_executed() * (lv.addrs[i] - target) * (lv.addrs[i] + wrap_at - target),
            );
        }
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &FullWordMemory<ExtensionTarget<D>> = vars.get_local_values().into();
        let is_executed = builder.add_extension(lv.ops.is_load, lv.ops.is_store);

        is_binary_ext_circuit(builder, lv.ops.is_store, yield_constr);
        is_binary_ext_circuit(builder, lv.ops.is_load, yield_constr);
        is_binary_ext_circuit(builder, is_executed, yield_constr);

        let wrap_at = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
        for i in 1..4 {
            let i_target = builder.constant_extension(F::Extension::from_canonical_usize(i));
            let target = builder.add_extension(lv.addrs[0], i_target);
            let addr_i_sub_target = builder.sub_extension(lv.addrs[i], target);
            let is_executed_mul_addr_i_sub_target =
                builder.mul_extension(is_executed, addr_i_sub_target);
            let addr_i_add_wrap_at = builder.add_extension(lv.addrs[i], wrap_at);
            let addr_i_add_wrap_at_sub_target = builder.sub_extension(addr_i_add_wrap_at, target);
            let constraint = builder.mul_extension(
                is_executed_mul_addr_i_sub_target,
                addr_i_add_wrap_at_sub_target,
            );
            yield_constr.constraint(builder, constraint);
        }
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{simple_test_code, u32_extra, u8_extra};
    use plonky2::plonk::config::Poseidon2GoldilocksConfig;
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    use starky::stark_testing::test_stark_circuit_constraints;

    use crate::memory_fullword::stark::FullWordMemoryStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    pub fn prove_mem_read_write<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u8) {
        let (program, record) = simple_test_code(
            &[
                Instruction {
                    op: Op::SW,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::LW,
                    args: Args {
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[
                (imm.wrapping_add(offset), 0),
                (imm.wrapping_add(offset).wrapping_add(1), 0),
                (imm.wrapping_add(offset).wrapping_add(2), 0),
                (imm.wrapping_add(offset).wrapping_add(3), 0),
            ],
            &[(1, content.into()), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_mem_read_write_mozak(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_mem_read_write::<MozakStark<F, D>>(offset, imm, content);
        }
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        type C = Poseidon2GoldilocksConfig;
        type S = FullWordMemoryStark<F, D>;
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
