//! This module implements constraints for the branch operations.

use expr::Expr;

use super::columns::CpuState;
use crate::expr::ConstraintBuilder;

/// Constraints for `less_than` and `normalised_diff`
/// For `less_than`:
///  `1` iff `r1 < r2`
///  `0` iff `r1 >= r2`
/// This holds when r1, r2 are signed or unsigned.
///
/// For `normalised_diff`:
///  `0` iff `r1 == r2`
///  `1` iff `r1 != r2`
pub(crate) fn comparison_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let lt = lv.less_than;
    cb.always(lt.is_binary());

    // We add inequality constraints, so that if:
    // `|r1 - r2| != r1 - r2`, then lt == 0
    // `|r1 - r2| != r2 - r1`, then lt == 1
    // However, this is still insufficient, as if |r1 - r2| == 0,
    // `lt` is not constrained and can also be 1, though it should only be 0.
    cb.always((1 - lt) * (lv.abs_diff - lv.signed_diff()));
    cb.always(lt * (lv.abs_diff + lv.signed_diff()));

    // Thus, we need a constraint when |r1 - r2| == 0 -> lt == 0.

    // To do so, we constrain `normalised_diff` to be
    //  0 iff r1 == r2
    //  1 iff r1 != r2
    cb.always(lv.normalised_diff.is_binary());
    cb.always(lv.signed_diff() * (1 - lv.normalised_diff));
    cb.always(lv.signed_diff() * lv.cmp_diff_inv - lv.normalised_diff);

    // Finally, we constrain so that only one of both `lt` and `normalised_diff`
    // can equal 1 at once. There for, if `op1 == op2`, then `normalised_diff == 1`,
    // thus `lt` can only be 0. Which means we are no longer under constrained.
    cb.always(lt * (1 - lv.normalised_diff));
}

/// Constraints for conditional branch operations
pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let ops = &lv.inst.ops;
    let is_blt = ops.blt;
    let is_bge = ops.bge;

    let bumped_pc = lv.inst.pc + 4;
    let branched_pc = lv.inst.imm_value;
    let next_pc = lv.new_pc;

    let lt = lv.less_than;

    // Check: for BLT and BLTU branch if `lt == 1`, otherwise just increment the pc.
    // Note that BLT and BLTU behave equivalently, as `lt` handles signed
    // conversions.
    cb.always(is_blt * lt * (next_pc - branched_pc));
    cb.always(is_blt * (1 - lt) * (next_pc - bumped_pc));

    // Check: for BGE and BGEU we reverse the checks of BLT and BLTU.
    cb.always(is_bge * lt * (next_pc - bumped_pc));
    cb.always(is_bge * (1 - lt) * (next_pc - branched_pc));

    // Check: for BEQ, branch if `normalised_diff == 0`, otherwise increment the pc.
    cb.always(ops.beq * (1 - lv.normalised_diff) * (next_pc - branched_pc));
    cb.always(ops.beq * lv.normalised_diff * (next_pc - bumped_pc));

    // Check: for BNE, we reverse the checks of BEQ.
    cb.always(ops.bne * lv.normalised_diff * (next_pc - branched_pc));
    cb.always(ops.bne * (1 - lv.normalised_diff) * (next_pc - bumped_pc));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::u32_extra;
    use proptest::prelude::ProptestConfig;
    use proptest::strategy::Just;
    use proptest::{prop_oneof, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_cond_branch<Stark: ProveAndVerify>(a: u32, b: u32, op: Op) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op,
                    args: Args {
                        rd: 0,
                        rs1: 6,
                        rs2: 7,
                        imm: 8, // branch target
                    },
                },
                // if above branch is not taken R1 has value 10.
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: 1,
                        imm: 10,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(6, a), (7, b)],
        );
        let taken = match op {
            Op::BLT => (a as i32) < (b as i32),
            Op::BLTU => a < b,
            Op::BGE => (a as i32) >= (b as i32),
            Op::BGEU => a >= b,
            Op::BEQ => a == b,
            Op::BNE => a != b,
            _ => unreachable!(),
        };
        assert_eq!(
            record.state_before_final().get_register_value(1),
            if taken { 0 } else { 10 }
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn prove_branch_cpu(a in u32_extra(), b in u32_extra(), op in prop_oneof![Just(Op::BLT), Just(Op::BLTU), Just(Op::BGE), Just(Op::BGEU), Just(Op::BEQ), Just(Op::BNE)]) {
            prove_cond_branch::<CpuStark<F, D>>(a, b, op);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_branch_mozak(a in u32_extra(), b in u32_extra(), op in prop_oneof![Just(Op::BLT), Just(Op::BLTU), Just(Op::BGE), Just(Op::BGEU), Just(Op::BEQ), Just(Op::BNE)]) {
            prove_cond_branch::<MozakStark<F, D>>(a, b, op);
        }
    }
}
