use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;
use super::stark::is_binary;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let op1_abs = lv.op1_abs;
    let op2_abs = lv.op2_abs;
    let low_limb = lv.product_low_limb;
    let high_limb = lv.product_high_limb;
    let two_to_32 = CpuState::<P>::shifted(32);
    let is_mul_op = lv.inst.ops.mul + lv.inst.ops.mulhu + lv.inst.ops.mulh + lv.inst.ops.mulhsu;

    // Make sure product_sign is either 0 or 1.
    is_binary(yield_constr, lv.product_sign);
    yield_constr.constraint(lv.product_zero * lv.op1_abs * lv.op2_abs);
    yield_constr.constraint(
        (P::ONES - lv.product_sign) * (high_limb * two_to_32 + low_limb - op1_abs * op2_abs),
    );
    yield_constr.constraint(
        lv.product_sign * (two_to_32 * (two_to_32 - high_limb) - low_limb - op1_abs * op2_abs),
    );
    // Make sure high_limb is not zero when product_sign is 1.
    yield_constr.constraint(lv.product_sign * (P::ONES - high_limb * lv.product_high_limb_inv));
    // Make sure op1_abs is computed correctly from op1_value.
    yield_constr.constraint(
        op1_abs
            - ((P::ONES - lv.op1_sign_bit) * lv.op1_value
                + lv.op1_sign_bit * (two_to_32 - lv.op1_value)),
    );
    // Make sure op2_abs is computed correctly from op2_value.
    yield_constr.constraint(
        is_mul_op
            * (op2_abs
                - ((P::ONES - lv.op2_sign_bit) * lv.op2_value
                    + lv.op2_sign_bit * (two_to_32 - lv.op2_value))),
    );
    // For MUL/MULHU/SLL product sign should alwasy be 0.
    yield_constr
        .constraint((lv.inst.ops.sll + lv.inst.ops.mul + lv.inst.ops.mulhu) * lv.product_sign);
    // Make sure product_sign is computed correctly.
    yield_constr.constraint(
        (P::ONES - lv.product_zero)
            * (lv.product_sign
                - ((lv.op1_sign_bit + lv.op2_sign_bit)
                    - (P::Scalar::from_canonical_u32(2) * lv.op1_sign_bit * lv.op2_sign_bit))),
    );
    // The following constraints are for SLL.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            lv.inst.ops.sll * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.input_b - op2));

        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(lv.inst.ops.sll * (op2_abs - lv.bitshift.multiplier));
    }

    // Now, let's copy our results to the destination register:
    let destination = lv.dst_value;
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));
    yield_constr.constraint(
        (lv.inst.ops.mulh + lv.inst.ops.mulhsu + lv.inst.ops.mulhu) * (destination - high_limb),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{i32_extra, reg, simple_test_code, u32_extra};
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::{prop_assert_eq, proptest};
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::cpu::stark::CpuStark;
    use crate::generation::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
    use crate::generation::program::generate_program_rom_trace;
    use crate::stark::mozak_stark::PublicInputs;
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{standard_faster_config, ProveAndVerify, C, D, F};
    use crate::utils::from_u32;
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    #[test]
    fn prove_mulhsu_example() {
        type S = CpuStark<F, D>;
        let config = standard_faster_config();
        let a = -2_147_451_028_i32;
        let b = 2_147_483_648_u32;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::MULHSU,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a as u32), (7, b)],
        );
        let res = i64::from(a).wrapping_mul(i64::from(b));
        assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        let mut timing = TimingTree::new("mulhsu", log::Level::Debug);
        let cpu_trace = timed!(
            timing,
            "generate_cpu_trace",
            generate_cpu_trace(&program, &record)
        );
        let trace_poly_values = timed!(
            timing,
            "trace to poly",
            trace_to_poly_values(generate_cpu_trace_extended(
                cpu_trace,
                &generate_program_rom_trace(&program)
            ))
        );
        let stark = S::default();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let proof = timed!(
            timing,
            "cpu proof",
            prove_table::<F, C, S, D>(
                stark,
                &config,
                trace_poly_values,
                public_inputs.into(),
                &mut timing,
            )
        );
        let proof = proof.unwrap();
        let verification_res = timed!(
            timing,
            "cpu verification",
            verify_stark_proof(stark, proof, &config)
        );
        verification_res.unwrap();
        timing.print();
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_mul_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MUL,
                        args: Args {
                            rd: 8,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            let (low, _high) = a.widening_mul(b);
            prop_assert_eq!(record.executed[0].aux.dst_val, low);
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
        #[test]
        fn prove_mulhu_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MULHU,
                        args: Args {
                            rd: 9,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            let (_low, high) = a.widening_mul(b);
            prop_assert_eq!(record.executed[0].aux.dst_val, high);
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulh_proptest(a in i32_extra(), b in i32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MULH,
                        args: Args {
                            rd: 8,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a as u32), (7, b as u32)],
            );
            let (res, overflow) = i64::from(a).overflowing_mul(i64::from(b));
            assert!(!overflow);
            prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulhsu_proptest(a in i32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MULHSU,
                        args: Args {
                            rd: 8,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a as u32), (7, b)],
            );
            let (res, _overflow) = i64::from(a).overflowing_mul(i64::from(b));
            prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }

        #[test]
        fn prove_sll_proptest(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs1 != rd);
            prop_assume!(rs2 != rd);
            let (program, record) = simple_test_code(
                &[Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd,
                        rs1,
                        imm: q,
                        ..Args::default()
                    },
                }
                ],
                &[],
                &[(rs1, p), (rs2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0b1_1111));
            prop_assert_eq!(record.executed[1].aux.dst_val, p << (q & 0b1_1111));
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
    }
}
