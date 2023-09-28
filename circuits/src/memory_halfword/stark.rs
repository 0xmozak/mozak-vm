use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use crate::cpu::stark::is_binary;
use crate::memory_halfword::columns::{HalfWordMemory, NUM_HW_MEM_COLS};

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct HalfWordMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for HalfWordMemoryStark<F, D> {
    const COLUMNS: usize = NUM_HW_MEM_COLS;
    const PUBLIC_INPUTS: usize = 0;

    // Design description - https://docs.google.com/presentation/d/1J0BJd49BMQh3UR5TrOhe3k67plHxnohFtFVrMpDJ1oc/edit?usp=sharing
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &HalfWordMemory<P> = vars.local_values.borrow();
        // let nv: &HalfWordMemory<P> = vars.next_values.borrow();

        is_binary(yield_constr, lv.is_sh);
        is_binary(yield_constr, lv.is_lhu);
        // TBD - why it is needed ???
        is_binary(yield_constr, lv.is_executed());

        // address-L1 == address + 1
        let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
        let added = lv.addr + P::ONES;
        let wrapped = added - wrap_at;

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        yield_constr
            .constraint(lv.is_executed() * (lv.addr_limb1 - added) * (lv.addr_limb1 - wrapped));
    }

    #[no_coverage]
    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }

    fn constraint_degree(&self) -> usize { 3 }
}

/// TODO: Roman - add tests
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use starky::stark_testing::test_stark_low_degree;

    use crate::memory_halfword::stark::HalfWordMemoryStark;
    use crate::memory_halfword::test_utils::halfword_memory_trace_test_case;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = HalfWordMemoryStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_memory_sh_lhu_all() -> Result<()> {
        let (program, executed) = halfword_memory_trace_test_case(1);
        MozakStark::prove_and_verify(&program, &executed)?;
        Ok(())
    }

    // #[test]
    // // fn prove_memory_sh_lhu() -> Result<()> {
    //     for repeats in 0..8 {
    //         // let (program, executed) =
    //         // halfword_memory_trace_test_case(repeats);
    //         // HalfWordMemoryStark::prove_and_verify(&program, &executed)?;
    //     }
    //     Ok(())
    // }
}
