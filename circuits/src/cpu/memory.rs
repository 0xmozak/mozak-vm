//! This module implements constraints for memory access, both for load and
//! store. Supported operators include: `SB` 'Save Byte', `LB` and `LBU` 'Load
//! Byte' and 'Load Byte Unsigned'

use expr::Expr;

use super::bitwise::and_gadget;
use super::columns::CpuState;
use crate::expr::ConstraintBuilder;

/// Ensure that `dst_value` and `mem_value_raw` only differ
/// in case of `LB` by `0xFFFF_FF00` and for `LH` by `0xFFFF_0000`. The
/// correctness of value presented in `dst_sign_bit` is ensured via range-check
pub(crate) fn signed_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.always(lv.dst_sign_bit.is_binary());
    // When dst is not signed as per instruction semantics, dst_sign_bit must be 0.
    cb.always((1 - lv.inst.is_dst_signed) * lv.dst_sign_bit);

    // Ensure `dst_value` is `0xFFFF_FF00` greater than
    // `mem_access_raw` in case `dst_sign_bit` is set
    cb.always(lv.inst.ops.lb * (lv.dst_value - (lv.mem_value_raw + lv.dst_sign_bit * 0xFFFF_FF00)));

    // Ensure `dst_value` is `0xFFFF_0000` greater than
    // `mem_access_raw` in case `dst_sign_bit` is set
    cb.always(lv.inst.ops.lh * (lv.dst_value - (lv.mem_value_raw + lv.dst_sign_bit * 0xFFFF_0000)));

    let and_gadget = and_gadget(&lv.xor);
    // SB/SH uses only least significant 8/16 bit from RS1 register.
    cb.always((lv.inst.ops.sb + lv.inst.ops.sh) * (and_gadget.input_a - lv.op1_value));
    cb.always(lv.inst.ops.sb * (and_gadget.input_b - 0x0000_00FF));
    cb.always(lv.inst.ops.sh * (and_gadget.input_b - 0x0000_FFFF));
    cb.always(
        (lv.inst.ops.sb + lv.inst.ops.sh) * (and_gadget.doubled_output - 2 * lv.mem_value_raw),
    );
}

pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    // memory address is equal to rs2-value + imm (wrapping)
    cb.always(lv.inst.ops.is_mem_op() * (lv.mem_addr - lv.op2_value));
    // signed memory constraints
    signed_constraints(lv, cb);
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::u32_extra;
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_sb<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::SB,
                args: Args {
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[(b, 0)],
            &[(6, a), (7, b)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    /// Tests for `LB` and `LBU` assuming read memory location
    /// is part of static ELF (read-write memory address space)
    /// TODO: Further testing needs to be done for non-init
    /// memory locations.
    /// TODO: In future we should test any combination of load and store
    /// in any order to work.
    fn prove_lb_and_lbu<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::LB,
                    args: Args {
                        rs1: 6,
                        rs2: 7,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::LBU,
                    args: Args {
                        rs1: 6,
                        rs2: 7,
                        ..Args::default()
                    },
                },
            ],
            &[(b, 0)],
            &[(6, a), (7, b)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    fn prove_sb_lbu<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u32) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::SB,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::LBU,
                    args: Args {
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[(imm.wrapping_add(offset), 0)],
            &[(1, content), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    fn prove_sb_lb<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u32) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::SB,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::LB,
                    args: Args {
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[(imm.wrapping_add(offset), 0)],
            &[(1, content), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    fn prove_sh_lh<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u32) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::SH,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::LH,
                    args: Args {
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[(imm.wrapping_add(offset), 0)],
            &[(1, content), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_sb_cpu(a in u32_extra(), b in u32_extra()) {
            prove_sb::<CpuStark<F, D>>(a, b);
        }

        #[test]
        fn prove_lb_cpu(a in u32_extra(), b in u32_extra()) {
            prove_lb_and_lbu::<CpuStark<F, D>>(a, b);
        }

        #[test]
        fn prove_lb_mozak(a in u32_extra(), b in u32_extra()) {
            prove_lb_and_lbu::<MozakStark<F, D>>(a, b);
        }

        #[test]
        fn prove_sb_lbu_cpu(offset in u32_extra(), imm in u32_extra(), content in u32_extra()) {
            prove_sb_lbu::<CpuStark<F, D>>(offset, imm, content);
        }

        #[test]
        fn prove_sb_lb_cpu(offset in u32_extra(), imm in u32_extra(), content in u32_extra()) {
            prove_sb_lb::<CpuStark<F, D>>(offset, imm, content);
        }

        #[test]
        fn prove_sh_lh_cpu(offset in u32_extra(), imm in u32_extra(), content in u32_extra()) {
            prove_sh_lh::<CpuStark<F, D>>(offset, imm, content);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_sb_mozak(a in u32_extra(), b in u32_extra()) {
            prove_sb::<MozakStark<F, D>>(a, b);
        }

        #[test]
        fn prove_sb_lbu_mozak(offset in u32_extra(), imm in u32_extra(), content in u32_extra()) {
            prove_sb_lbu::<MozakStark<F, D>>(offset, imm, content);
        }

        #[test]
        fn prove_sb_lb_mozak(offset in u32_extra(), imm in u32_extra(), content in u32_extra()) {
            prove_sb_lb::<MozakStark<F, D>>(offset, imm, content);
        }

        #[test]
        fn prove_sh_lh_mozak(offset in u32_extra(), imm in u32_extra(), content in u32_extra()) {
            prove_sh_lh::<MozakStark<F, D>>(offset, imm, content);
        }
    }
}
