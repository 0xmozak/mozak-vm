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
use crate::memory_halfword::columns::{HalfWordMemory, NUM_HW_MEM_COLS};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct HalfWordMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for HalfWordMemoryStark<F, D> {
    type Columns = HalfWordMemory<F>;
}

const COLUMNS: usize = NUM_HW_MEM_COLS;
const PUBLIC_INPUTS: usize = 0;
impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for HalfWordMemoryStark<F, D> {
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
        let lv: &HalfWordMemory<P> = vars.get_local_values().into();

        is_binary(yield_constr, lv.ops.is_store);
        is_binary(yield_constr, lv.ops.is_load);
        is_binary(yield_constr, lv.is_executed());

        let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
        let added = lv.addrs[0] + P::ONES;
        let wrapped = added - wrap_at;

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        yield_constr.constraint(lv.is_executed() * (lv.addrs[1] - added) * (lv.addrs[1] - wrapped));
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &HalfWordMemory<ExtensionTarget<D>> = vars.get_local_values().into();
        let is_executed = builder.add_extension(lv.ops.is_load, lv.ops.is_store);

        is_binary_ext_circuit(builder, lv.ops.is_store, yield_constr);
        is_binary_ext_circuit(builder, lv.ops.is_load, yield_constr);
        is_binary_ext_circuit(builder, is_executed, yield_constr);

        let wrap_at = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
        let one = builder.one_extension();
        let added = builder.add_extension(lv.addrs[0], one);
        let wrapped = builder.sub_extension(added, wrap_at);

        let addr_1_sub_added = builder.sub_extension(lv.addrs[1], added);
        let addr_1_sub_wrapped = builder.sub_extension(lv.addrs[1], wrapped);
        let is_executed_mul_addr_1_sub_added = builder.mul_extension(is_executed, addr_1_sub_added);
        let constraint =
            builder.mul_extension(is_executed_mul_addr_1_sub_added, addr_1_sub_wrapped);

        yield_constr.constraint(builder, constraint);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{u32_extra, u8_extra};
    use mozak_runner::util::code::execute;
    use plonky2::plonk::config::Poseidon2GoldilocksConfig;
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    use starky::stark_testing::test_stark_circuit_constraints;

    use crate::memory_halfword::stark::HalfWordMemoryStark;
    // use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};
    pub fn prove_mem_read_write<Stark: ProveAndVerify>(
        offset: u32,
        imm: u32,
        content: u8,
        is_unsigned: bool,
    ) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::SH,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: if is_unsigned { Op::LHU } else { Op::LH },
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
        fn prove_mem_read_write_mozak(offset in u32_extra(), imm in u32_extra(), content in u8_extra(), is_unsigned: bool) {
            prove_mem_read_write::<MozakStark<F, D>>(offset, imm, content, is_unsigned);
        }
    }
    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        type C = Poseidon2GoldilocksConfig;
        type S = HalfWordMemoryStark<F, D>;
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
