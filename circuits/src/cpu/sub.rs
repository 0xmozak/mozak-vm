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
    let expected_value = lv.op1_value - lv.op2_value;
    let wrapped = P::Scalar::from_noncanonical_u64(1 << 32) + expected_value;

    // Check: the result of subtraction is wrapped if necessary.
    // As the result is range checked, this make the choice deterministic,
    // even for a malicious prover.
    yield_constr
        .constraint(lv.inst.ops.sub * ((lv.dst_value - expected_value) * (lv.dst_value - wrapped)));
}

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let expected_value = builder.sub_extension(lv.op1_value, lv.op2_value);
    let wrap_at = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let wrapped = builder.add_extension(wrap_at, expected_value);
    let dst_value_sub_expected_value = builder.sub_extension(lv.dst_value, expected_value);
    let dst_value_sub_wrapped = builder.sub_extension(lv.dst_value, wrapped);
    let dst_value_sub_expected_value_mul_dst_value_sub_wrapped =
        builder.mul_extension(dst_value_sub_expected_value, dst_value_sub_wrapped);
    let constr = builder.mul_extension(
        lv.inst.ops.sub,
        dst_value_sub_expected_value_mul_dst_value_sub_wrapped,
    );
    yield_constr.constraint(builder, constr);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_sub<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::SUB,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a), (7, b)],
        );
        assert_eq!(record.last_state.get_register_value(5), a.wrapping_sub(b));
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_sub_cpu(a in u32_extra(), b in u32_extra()) {
            prove_sub::<CpuStark<F, D>>(a, b);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_sub_mozak(a in u32_extra(), b in u32_extra()) {
            prove_sub::<MozakStark<F, D>>(a, b);
        }
    }
}
