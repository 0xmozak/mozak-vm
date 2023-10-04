//! This module implements constraints for memory access, both for load and
//! store. Supported operators include: `SB` 'Save Byte', `LB` and `LBU` 'Load
//! Byte' and 'Load Byte Unsigned'

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;
use crate::stark::utils::is_binary;

/// Ensure that `dst_value` and `mem_access_raw` only differ
/// in case of `LB` and only by `0xFFFF_FF00`. The correctness
/// of value presented in `dst_sign_bit` is ensured via range-check
pub(crate) fn signed_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.dst_sign_bit);
    // When dst is not signed as per instruction semantics, dst_sign_bit must be 0.
    yield_constr.constraint((P::ONES - lv.inst.is_dst_signed) * lv.dst_sign_bit);

    // Ensure `mem_access_raw` and `dst_value` are similar if unsigned operation
    yield_constr.constraint(
        lv.inst.ops.lb * (P::ONES - lv.inst.is_dst_signed) * (lv.dst_value - lv.mem_access_raw),
    );

    // Ensure `dst_value` is `0xFFFF_FF00` greater than
    // `mem_access_raw` in case `dst_sign_bit` is set
    yield_constr.constraint(
        lv.inst.ops.lb
            * lv.dst_sign_bit
            * (lv.dst_value - (lv.mem_access_raw + P::Scalar::from_canonical_u32(0xFFFF_FF00))),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{simple_test_code, u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_sb<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = simple_test_code(
            &[Instruction {
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

    fn prove_lb<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::LB,
                args: Args {
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a), (7, b)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    // NOTE: prove_lbu fails with MozakSnark
    fn prove_lbu<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::LBU,
                args: Args {
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a), (7, b)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    fn prove_mem_read_write<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u8) {
        let (program, record) = simple_test_code(
            &[
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
            &[(1, content.into()), (2, offset)],
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
            prove_lb::<CpuStark<F, D>>(a, b);
        }

        #[test]
        fn prove_lbu_cpu(a in u32_extra(), b in u32_extra()) {
            prove_lbu::<CpuStark<F, D>>(a, b);
        }

        #[test]
        fn prove_mem_read_write_cpu(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_mem_read_write::<CpuStark<F, D>>(offset, imm, content);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_sb_mozak(a in u32_extra(), b in u32_extra()) {
            prove_sb::<MozakStark<F, D>>(a, b);
        }

        #[test]
        fn prove_mem_read_write_mozak(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_mem_read_write::<MozakStark<F, D>>(offset, imm, content);
        }
    }
}
