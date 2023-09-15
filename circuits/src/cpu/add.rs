//! This module implements the constraints for the ADD operation.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

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

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_executor::instruction::{Args, Instruction, Op};
    use mozak_executor::test_utils::{reg, simple_test_code, u32_extra};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_add<Stark: ProveAndVerify>(a: u32, b: u32, rd: u8) {
        let (program, record) = simple_test_code(
            &[Instruction {
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
