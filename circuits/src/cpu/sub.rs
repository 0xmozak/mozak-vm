use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

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

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_executor::instruction::{Args, Instruction, Op};
    use mozak_executor::test_utils::{simple_test_code, u32_extra};
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
