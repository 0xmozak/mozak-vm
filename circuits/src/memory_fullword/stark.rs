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

use crate::memory_fullword::columns::{FullWordMemory, NUM_HW_MEM_COLS};
use crate::stark::utils::is_binary;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct FullWordMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
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

impl<F, const D: usize> Display for FullWordMemoryStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FullWordMemoryStark")
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{simple_test_code, u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

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
            &[(imm.wrapping_add(offset), 0)],
            &[(1, content.into()), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(1))]

            #[test]
            fn prove_mem_read_write_mozak(offset in u32_extra(), imm in
    u32_extra(), content in u8_extra()) {
    prove_mem_read_write::<MozakStark<F, D>>(offset, imm, content);         }
        }
}
