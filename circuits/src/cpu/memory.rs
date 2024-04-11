//! This module implements constraints for memory access, both for load and
//! store. Supported operators include: `SB` 'Save Byte', `LB` and `LBU` 'Load
//! Byte' and 'Load Byte Unsigned'

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::bitwise::{and_gadget, and_gadget_extension_targets};
use super::columns::{is_mem_op_extention_target, CpuState};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

/// Ensure that `dst_value` and `mem_value_raw` only differ
/// in case of `LB` by `0xFFFF_FF00` and for `LH` by `0xFFFF_0000`. The
/// correctness of value presented in `dst_sign_bit` is ensured via range-check
pub(crate) fn signed_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.dst_sign_bit);
    // When dst is not signed as per instruction semantics, dst_sign_bit must be 0.
    yield_constr.constraint((P::ONES - lv.inst.is_dst_signed) * lv.dst_sign_bit);

    // Ensure `dst_value` is `0xFFFF_FF00` greater than
    // `mem_access_raw` in case `dst_sign_bit` is set
    yield_constr.constraint(
        lv.inst.ops.lb
            * (lv.dst_value
                - (lv.mem_value_raw
                    + lv.dst_sign_bit * P::Scalar::from_canonical_u32(0xFFFF_FF00))),
    );

    // Ensure `dst_value` is `0xFFFF_0000` greater than
    // `mem_access_raw` in case `dst_sign_bit` is set
    yield_constr.constraint(
        lv.inst.ops.lh
            * (lv.dst_value
                - (lv.mem_value_raw
                    + lv.dst_sign_bit * P::Scalar::from_canonical_u32(0xFFFF_0000))),
    );

    let and_gadget = and_gadget(&lv.xor);
    // SB/SH uses only least significant 8/16 bit from RS1 register.
    yield_constr
        .constraint((lv.inst.ops.sb + lv.inst.ops.sh) * (and_gadget.input_a - lv.op1_value));
    yield_constr.constraint(
        lv.inst.ops.sb * (and_gadget.input_b - P::Scalar::from_canonical_u32(0x0000_00FF)),
    );
    yield_constr.constraint(
        lv.inst.ops.sh * (and_gadget.input_b - P::Scalar::from_canonical_u32(0x0000_FFFF)),
    );
    yield_constr
        .constraint((lv.inst.ops.sb + lv.inst.ops.sh) * (and_gadget.output - lv.mem_value_raw));
}

pub(crate) fn signed_constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    is_binary_ext_circuit(builder, lv.dst_sign_bit, yield_constr);
    let one = builder.one_extension();
    let one_sub_dst_signed = builder.sub_extension(one, lv.inst.is_dst_signed);
    let constr = builder.mul_extension(one_sub_dst_signed, lv.dst_sign_bit);
    yield_constr.constraint(builder, constr);

    let ffff_ff00 = builder.constant_extension(F::Extension::from_canonical_u64(0xFFFF_FF00));
    let dst_sign_bit_mul_ffff_ff00 = builder.mul_extension(lv.dst_sign_bit, ffff_ff00);
    let mem_value_raw_add_dst_sign_bit_mul_ffff_ff00 =
        builder.add_extension(lv.mem_value_raw, dst_sign_bit_mul_ffff_ff00);
    let dst_value_sub_mem_value_raw_add_dst_sign_bit_mul_ffff_ff00 =
        builder.sub_extension(lv.dst_value, mem_value_raw_add_dst_sign_bit_mul_ffff_ff00);
    let constr = builder.mul_extension(
        lv.inst.ops.lb,
        dst_value_sub_mem_value_raw_add_dst_sign_bit_mul_ffff_ff00,
    );
    yield_constr.constraint(builder, constr);

    let ffff_0000 = builder.constant_extension(F::Extension::from_canonical_u64(0xFFFF_0000));
    let dst_sign_bit_mul_ffff_0000 = builder.mul_extension(lv.dst_sign_bit, ffff_0000);
    let mem_value_raw_add_dst_sign_bit_mul_ffff_0000 =
        builder.add_extension(lv.mem_value_raw, dst_sign_bit_mul_ffff_0000);
    let dst_value_sub_mem_value_raw_add_dst_sign_bit_mul_ffff_0000 =
        builder.sub_extension(lv.dst_value, mem_value_raw_add_dst_sign_bit_mul_ffff_0000);
    let constr = builder.mul_extension(
        lv.inst.ops.lh,
        dst_value_sub_mem_value_raw_add_dst_sign_bit_mul_ffff_0000,
    );
    yield_constr.constraint(builder, constr);

    let and_gadget = and_gadget_extension_targets(builder, &lv.xor);
    let sb_add_sh = builder.add_extension(lv.inst.ops.sb, lv.inst.ops.sh);
    let and_input_a_sub_op1_value = builder.sub_extension(and_gadget.input_a, lv.op1_value);
    let constr = builder.mul_extension(sb_add_sh, and_input_a_sub_op1_value);
    yield_constr.constraint(builder, constr);

    let num = builder.constant_extension(F::Extension::from_canonical_u64(0x0000_00FF));
    let and_input_b_sub_num = builder.sub_extension(and_gadget.input_b, num);
    let constr = builder.mul_extension(lv.inst.ops.sb, and_input_b_sub_num);
    yield_constr.constraint(builder, constr);

    let num = builder.constant_extension(F::Extension::from_canonical_u64(0x0000_FFFF));
    let and_input_b_sub_num = builder.sub_extension(and_gadget.input_b, num);
    let constr = builder.mul_extension(lv.inst.ops.sh, and_input_b_sub_num);
    yield_constr.constraint(builder, constr);

    let and_output_sub_mem_value_raw = builder.sub_extension(and_gadget.output, lv.mem_value_raw);
    let constr = builder.mul_extension(sb_add_sh, and_output_sub_mem_value_raw);
    yield_constr.constraint(builder, constr);
}

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // memory address is equal to rs2-value + imm (wrapping)
    yield_constr.constraint(lv.inst.ops.is_mem_ops() * (lv.mem_addr - lv.op2_value));
    // signed memory constraints
    signed_constraints(lv, yield_constr);
}

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let is_mem_ops = is_mem_op_extention_target(builder, &lv.inst.ops);
    let mem_addr_sub_op2_value = builder.sub_extension(lv.mem_addr, lv.op2_value);
    let constr = builder.mul_extension(is_mem_ops, mem_addr_sub_op2_value);
    yield_constr.constraint(builder, constr);

    signed_constraints_circuit(builder, lv, yield_constr);
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
