//! This module implements the constraints for the ADD operation.

use expr::Expr;

use super::columns::CpuState;
use crate::expr::ConstraintBuilder;

pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let added = lv.op1_value + lv.op2_value;
    let wrapped = added - (1 << 32);

    // Check: the resulting sum is wrapped if necessary.
    // As the result is range checked, this make the choice deterministic,
    // even for a malicious prover.
    cb.always(lv.inst.ops.add * (lv.dst_value - added) * (lv.dst_value - wrapped));
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{reg, u32_extra};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_add<Stark: ProveAndVerify>(a: u32, b: u32, rd: u8) {
        let (program, record) = code::execute(
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

    #[test]
    fn prove_add_mozak_example() {
        let a = 1;
        let b = 2;
        let rd = 3;
        prove_add::<MozakStark<F, D>>(a, b, rd);
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
