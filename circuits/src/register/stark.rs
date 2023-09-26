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

    /// Constraints for the [`RegisterStark`]:
    ///
    /// 1) Trace should start with register address 1 - we exclude 0 for ease of
    ///    CTLs.
    /// 2) `is_init`, `is_read`, `is_write`, and the virtual `is_dummy` column
    ///    are binary columns.
    /// 3) `is_dummy` only take values 0 or 1.
    /// 4) Only rd changes.
    /// 5) Address changes only when nv.is_init == 1.
    /// 6) Address either stays the same or increments by 1.
    /// 7) Trace should end with register address 31.
    ///
    /// For more details, refer to the [Notion
    /// document](https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2).
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
        let local_dummy = lv.is_init + lv.is_read + lv.is_write;
        let next_dummy = nv.is_init + nv.is_read + nv.is_write;

        // Constraint 1: trace rows starts with register address 1.
        yield_constr.constraint_first_row(lv.addr - P::ONES);

        // Constraint 2: filter columns take 0 or 1 values only.
        is_binary(yield_constr, lv.is_init);
        is_binary(yield_constr, lv.is_read);
        is_binary(yield_constr, lv.is_write);
        is_binary(yield_constr, local_dummy);

        // Constraint 3: virtual dummy column can only take values 0 or 1.
        // (local_dummy - next_dummy - 1) is expressed as such, because
        // local_dummy = 1 in the last real row, and
        // next_dummy = 0 in the first padding row.
        yield_constr.constraint_transition(
            (next_dummy - local_dummy) * (local_dummy - next_dummy - P::ONES),
        );

        // Constraint 4: only rd changes.
        // We reformulate the above constraint as such:
        // For any register, only `is_write`, `is_init` or `is_dummy`
        // should be able to change the values.
        // `is_read` should not change the value of the
        // register.
        yield_constr.constraint_transition(
            lv.is_read * (nv.value - lv.value) * (nv.addr - lv.addr - P::ONES),
        );

        // Constraint 5: Address changes only when nv.is_init == 1.
        // We reformulate the above constraint to be:
        // if next `is_read` == 1 or next `is_write` == 1, the address cannot
        // change.
        yield_constr.constraint_transition((nv.is_read + nv.is_write) * (nv.addr - lv.addr));

        // Constraint 6: Address either stays the same or increments by 1.
        yield_constr.constraint_transition((nv.addr - lv.addr) * (nv.addr - lv.addr - P::ONES));

        // Constraint 7: last register address == 31
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
