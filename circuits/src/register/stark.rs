use std::borrow::Borrow;
use std::fmt::Display;
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

impl<F, const D: usize> Display for RegisterStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegisterStark")
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RegisterStark<F, D> {
    const COLUMNS: usize = Register::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    /// Constraints for the [`RegisterStark`]:
    ///
    /// 1) Trace should start with register address 1 - we exclude 0 for ease of
    ///    CTLs.
    /// 2) `is_init`, `is_read`, `is_write`, and the virtual `is_used` column
    ///    are binary columns. The `is_used` column is the sum of all the other
    ///    filter columns combined, to differentiate between real trace rows and
    ///    padding rows.
    /// 3) The virtual `is_used` column only take values 0 or 1.
    /// 4) Only rd changes.
    /// 5) Address changes only when `nv.is_init` == 1.
    /// 6) Address either stays the same or increments by 1.
    /// 7) `augmented_clk` is 0 for all `is_init` rows. 
    /// 8) Trace should end with register address 31.
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

        // We create a virtual column known as `is_used`, which flags a row as
        // being 'used' if it any one of the filter columns are turned on.
        // This is to differentiate between real rows and padding rows.
        let local_is_used = lv.ops.is_used();
        let next_is_used = nv.ops.is_used();

        // Constraint 2: filter columns take 0 or 1 values only.
        is_binary(yield_constr, lv.ops.is_init);
        is_binary(yield_constr, lv.ops.is_read);
        is_binary(yield_constr, lv.ops.is_write);
        is_binary(yield_constr, local_is_used);

        // Constraint 3: virtual `is_used` column can only take values 0 or 1.
        // (local_is_used - next_is_used - 1) is expressed as such, because
        // local_is_used = 1 in the last real row, and
        // next_is_used = 0 in the first padding row.
        yield_constr.constraint_transition(
            (next_is_used - local_is_used) * (local_is_used - next_is_used - P::ONES),
        );

        // Constraint 4: only rd changes.
        // We reformulate the above constraint as such:
        // For any register, only `is_write`, `is_init` or the virtual `is_used`
        // column should be able to change values of registers.
        // `is_read` should not change the values of registers.
        yield_constr.constraint_transition(nv.ops.is_read * (nv.value - lv.value));

        // Constraint 5: Address changes only when nv.is_init == 1.
        // We reformulate the above constraint to be:
        // if next `is_read` == 1 or next `is_write` == 1, the address cannot
        // change.
        yield_constr
            .constraint_transition((nv.ops.is_read + nv.ops.is_write) * (nv.addr - lv.addr));

        // Constraint 6: Address either stays the same or increments by 1.
        yield_constr.constraint_transition((nv.addr - lv.addr) * (nv.addr - lv.addr - P::ONES));

        // Constraint 7: `augmented_clk` is 0 for all `is_init` rows. 
        yield_constr.constraint(lv.ops.is_init * lv.augmented_clk);

        // This combines 2 constraints,
        //   a) Constraint 1: trace rows starts with register address 1,
        //   b) Constraint 8: last register address == 31,
        // by using the fact that `vars.next_values` wrap around.
        yield_constr.constraint_last_row(lv.addr - nv.addr - P::from(FE::from_canonical_u8(30)));
    }

    #[coverage(off)]
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
    use mozak_runner::test_utils::{reg, simple_test_code, u32_extra};
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

    fn prove_stark<Stark: ProveAndVerify>(a: u32, b: u32, imm: u32, rd: u8) {
        let (program, record) = simple_test_code(
            &[
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd,
                        rs1: 6,
                        rs2: 7,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd,
                        rs1: 6,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(6, a), (7, b)],
        );
        if rd != 0 {
            assert_eq!(
                record.executed[1].state.get_register_value(rd),
                a.wrapping_add(b)
            );
        }
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_register(a in u32_extra(), b in u32_extra(), imm in u32_extra(), rd in reg()) {
            prove_stark::<RegisterStark<F, D>>(a, b, imm, rd);
        }
    }
}
