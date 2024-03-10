//! This module implements the constraints for the ADD operation.

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
    let added = lv.op1_value + lv.op2_value;
    let wrapped = added - wrap_at;

    // Check: the resulting sum is wrapped if necessary.
    // As the result is range checked, this make the choice deterministic,
    // even for a malicious prover.
    yield_constr.constraint(lv.inst.ops.add * (lv.dst_value - added) * (lv.dst_value - wrapped));
}

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let wrap_at = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let added = builder.add_extension(lv.op1_value, lv.op2_value);
    let wrapped = builder.sub_extension(added, wrap_at);
    let dst_value_sub_added = builder.sub_extension(lv.dst_value, added);
    let dst_value_sub_wrapped = builder.sub_extension(lv.dst_value, wrapped);
    let dst_value_sub_added_mul_dst_value_sub_wrapped =
        builder.mul_extension(dst_value_sub_added, dst_value_sub_wrapped);
    let constr = builder.mul_extension(
        lv.inst.ops.add,
        dst_value_sub_added_mul_dst_value_sub_wrapped,
    );
    yield_constr.constraint(builder, constr);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{reg, u32_extra};
    use mozak_runner::util::execute_code;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_add<Stark: ProveAndVerify>(a: u32, b: u32, rd: u8) {
        let (program, record) = execute_code(
            [Instruction {
                op: Op::ADD,
                args: Args {
                    rd,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
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
        fn prove_add_cpu(a in u32_extra(), b in u32_extra(), rd in reg()) {
            prove_add::<CpuStark<F, D>>(a, b, rd);
        }
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_add_mozak(a in u32_extra(), b in u32_extra(), rd in reg()) {
            prove_add::<MozakStark<F, D>>(a, b, rd);
        }
    }
}
