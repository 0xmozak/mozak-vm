use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
    let addr = lv.op2_value;
    let rs2_value = addr - lv.inst.imm_value;

    let wrapped = wrap_at + rs2_value;

    yield_constr.constraint(
        lv.inst.ops.is_mem_op() * (addr - rs2_value - lv.inst.imm_value) * (addr - wrapped),
    );

    // TODO: support for SH / SW
    let sh_offset = P::Scalar::from_noncanonical_u64(1);
    let addr = lv.op2_value + sh_offset;
    yield_constr.constraint(
        lv.inst.ops.sh
            * (addr - rs2_value - lv.inst.imm_value - sh_offset)
            * (addr - wrapped - sh_offset),
    );

    let sw_offset = P::Scalar::from_noncanonical_u64(3);
    let addr = lv.op2_value + sw_offset;
    yield_constr.constraint(
        lv.inst.ops.sw
            * (addr - rs2_value - lv.inst.imm_value - sw_offset)
            * (addr - wrapped - sw_offset),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;

    fn standard_instruction(op: Op) -> Instruction {
        Instruction {
            op,
            args: Args {
                rs1: 6,
                rs2: 7,
                ..Args::default()
            },
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_sb_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[standard_instruction(Op::SB)],
                &[],
                &[(6, a), (7, b)],
            );

            CpuStark::prove_and_verify(&program, &record).unwrap();
        }

        #[test]
        fn prove_mem_read_write_proptest(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
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
                &[],
                &[(1, content.into()), (2, offset)],
            );

            CpuStark::prove_and_verify(&program, &record).unwrap();
        }

        fn prove_sh_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[standard_instruction(Op::SH)],
                &[],
                &[(6, a), (7, b)],
            );

            CpuStark::prove_and_verify(&program, &record).unwrap();
        }

        #[test]
        fn prove_sw_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[standard_instruction(Op::SW)],
                &[],
                &[(6, a), (7, b)],
            );

            CpuStark::prove_and_verify(&program, &record).unwrap();
        }


        #[test]
        fn prove_lbu_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[standard_instruction(Op::LBU)],
                &[],
                &[(6, a), (7, b)],
            );

            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
    }
}
