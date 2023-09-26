use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::Register;
use crate::columns_view::NumberOfColumns;
use crate::cpu::stark::is_binary;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RegisterStark<F, D> {
    const COLUMNS: usize = Register::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    /// Constraints for the [`RegisterStark`].
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &Register<P> = vars.local_values.borrow();
        let nv: &Register<P> = vars.next_values.borrow();

        // Virtual dummy column to differentiate between real rows and padding rows.
        // TODO: CTL dummy column against `RegisterInit`
        let local_dummy = lv.is_init + lv.is_read + lv.is_write;
        let next_dummy = nv.is_init + nv.is_read + nv.is_write;

        // Check: first register address == 1, i.e. we do not have register address 0
        // in our trace.
        yield_constr.constraint_first_row(lv.addr - P::ONES);

        // Check: filter columns take 0 or 1 values only.
        is_binary(yield_constr, lv.is_init);
        is_binary(yield_constr, lv.is_read);
        is_binary(yield_constr, lv.is_write);
        is_binary(yield_constr, local_dummy);

        // Check: virtual dummy column can flip between 1 or 0
        // (local_dummy - next_dummy - 1) is expressed as such, because
        // local_dummy = 1 in the last real row, and
        // next_dummy = 0 in the first padding row.
        yield_constr.constraint_transition((next_dummy - local_dummy) * (local_dummy - next_dummy - P::ONES));


        // Only when next row is an init row, i.e. `is_init` == 1,
        // then the register entry can change address.
        yield_constr.constraint_transition((nv.is_read + nv.is_write) * (nv.addr - lv.addr));

        // Check: for any register, `is_read` should not change the value of the
        // register. Only `is_write`, `is_init` or `is_dummy` should be able to
        // change the values.
        yield_constr.constraint_transition(
            lv.is_read * (nv.value - lv.value) * (nv.addr - lv.addr - P::ONES),
        );

        // Check: last register address == 31
        yield_constr.constraint_last_row(lv.addr - P::from(FE::from_canonical_u8(31)));
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use starky::stark_testing::test_stark_low_degree;

    use super::*;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RegisterStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_reg() -> Result<()> {
        let instructions = [
            Instruction::new(Op::ADD, Args {
                rs1: 6,
                rs2: 7,
                rd: 4,
                ..Args::default()
            }),
            Instruction::new(Op::ADD, Args {
                rs1: 4,
                rs2: 6,
                rd: 5,
                ..Args::default()
            }),
            Instruction::new(Op::ADD, Args {
                rs1: 5,
                rd: 4,
                imm: 100,
                ..Args::default()
            }),
        ];

        let (program, record) = simple_test_code(&instructions, &[], &[(6, 100), (7, 200)]);
        RegisterStark::prove_and_verify(&program, &record)?;
        Ok(())
    }
}
