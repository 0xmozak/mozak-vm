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

use super::columns::Register;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for RegisterStark<F, D> {
    type Columns = Register<F>;
}

const COLUMNS: usize = Register::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 2;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RegisterStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    /// Constraints for the [`RegisterStark`]:
    ///
    /// 1) `is_init`, `is_read`, `is_write`, and the virtual `is_used` column
    ///    are binary columns. The `is_used` column is the sum of all the other
    ///    ops columns combined, to differentiate between real trace rows and
    ///    padding rows.
    /// 2) The virtual `is_used` column only take values 0 or 1.
    /// 3) Only rd changes.
    /// 4) Address changes only when `nv.is_init` == 1.
    /// 5) Address either stays the same or increments by 1.
    /// 6) Addresses go from 1 to 31.  Address 0 is handled by
    ///    `RegisterZeroStark`.
    ///
    /// For more details, refer to the [Notion
    /// document](https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2).
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &Register<P> = vars.get_local_values().into();
        let nv: &Register<P> = vars.get_next_values().into();

        // Constraint 1: filter columns take 0 or 1 values only.
        is_binary(yield_constr, lv.ops.is_init);
        is_binary(yield_constr, lv.ops.is_read);
        is_binary(yield_constr, lv.ops.is_write);
        is_binary(yield_constr, lv.is_used());

        // Constraint 2: virtual `is_used` column can only take values 0 or 1.
        // (lv.is_used() - nv.is_used() - 1) is expressed as such, because
        // lv.is_used() = 1 in the last real row, and
        // nv.is_used() = 0 in the first padding row.
        yield_constr.constraint_transition(nv.is_used() * (nv.is_used() - lv.is_used()));

        // Constraint 3: only rd changes.
        // We reformulate the above constraint as such:
        // For any register, only `is_write`, `is_init` or the virtual `is_used`
        // column should be able to change values of registers.
        // `is_read` should not change the values of registers.
        yield_constr.constraint_transition(nv.ops.is_read * (nv.value - lv.value));

        // Constraint 4: Address changes only when nv.is_init == 1.
        // We reformulate the above constraint to be:
        // if next `is_read` == 1 or next `is_write` == 1, the address cannot
        // change.
        yield_constr
            .constraint_transition((nv.ops.is_read + nv.ops.is_write) * (nv.addr - lv.addr));

        // Constraint 5: Address either stays the same or increments by 1.
        yield_constr.constraint_transition((nv.addr - lv.addr) * (nv.addr - lv.addr - P::ONES));

        // Constraint 6: addresses go from 1 to 31.
        yield_constr.constraint_first_row(lv.addr - P::ONES);
        yield_constr.constraint_last_row(lv.addr - P::Scalar::from_canonical_u8(31));
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &Register<_> = vars.get_local_values().into();
        let nv: &Register<_> = vars.get_next_values().into();

        let nv_is_used = nv.ops.iter().fold(builder.zero_extension(), |acc, s| {
            builder.add_extension(acc, *s)
        });
        // TODO: extract summing function and use it in `cpu/stark.rs`, too.
        let lv_is_used = lv.ops.iter().fold(builder.zero_extension(), |acc, s| {
            builder.add_extension(acc, *s)
        });

        let addr_diff = builder.sub_extension(nv.addr, lv.addr);
        let one = builder.one_extension();

        // Constraint 1.
        {
            is_binary_ext_circuit(builder, lv.ops.is_init, yield_constr);
            is_binary_ext_circuit(builder, lv.ops.is_read, yield_constr);
            is_binary_ext_circuit(builder, lv.ops.is_write, yield_constr);
            is_binary_ext_circuit(builder, lv_is_used, yield_constr);
        }

        // Constraint 2.
        {
            let is_used_diff = builder.sub_extension(nv_is_used, lv_is_used);
            let disjunction = builder.mul_extension(nv_is_used, is_used_diff);
            yield_constr.constraint_transition(builder, disjunction);
        }

        // Constraint 3.
        {
            let rd_diff = builder.sub_extension(nv.value, lv.value);
            let disjunction = builder.mul_extension(nv.ops.is_read, rd_diff);
            yield_constr.constraint_transition(builder, disjunction);
        }

        // Constraint 4.
        {
            let aint_init = builder.add_extension(nv.ops.is_read, nv.ops.is_write);
            let disjunction = builder.mul_extension(aint_init, addr_diff);
            yield_constr.constraint_transition(builder, disjunction);
        }

        // Constraint 5.
        {
            let addr_diff = builder.sub_extension(nv.addr, lv.addr);

            let addr_diff_sub_one = builder.sub_extension(addr_diff, one);
            let addr_diff_mul_addr_diff_sub_one =
                builder.mul_extension(addr_diff, addr_diff_sub_one);
            yield_constr.constraint_transition(builder, addr_diff_mul_addr_diff_sub_one);
        }

        // Constraint 6.
        {
            let lv_addr_sub_one = builder.sub_extension(lv.addr, one);
            yield_constr.constraint_first_row(builder, lv_addr_sub_one);
            let v31 = builder.constant_extension(F::Extension::from_canonical_u8(31));
            let lv_addr_sub_v31 = builder.sub_extension(lv.addr, v31);
            yield_constr.constraint_last_row(builder, lv_addr_sub_v31);
        }
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{reg, u32_extra};
    use mozak_runner::util::execute_code;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use super::*;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RegisterStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn test_circuit() -> Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)
    }

    fn prove_stark<Stark: ProveAndVerify>(a: u32, b: u32, imm: u32, rd: u8) {
        let (program, record) = execute_code(
            [
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
