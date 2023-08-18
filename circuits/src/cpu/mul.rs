use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // The Goldilocks field is carefully chosen to allow multiplication of u32
    // values without overflow.
    let base = P::Scalar::from_noncanonical_u64(1 << 32);

    let multiplier_abs = lv.multiplier_abs;
    let multiplicand_abs = lv.multiplicand_abs;
    let low_limb = lv.product_low_bits;
    let high_limb = lv.product_high_bits;
    let product = low_limb + base * high_limb;

    yield_constr.constraint(product - multiplicand_abs * multiplier_abs);
    // Make sure multiplier_abs is computed correctly from op2_value.
    // Skip SLL as it always has positive multiplier.
    yield_constr.constraint(
        (lv.inst.ops.mul + lv.inst.ops.mulhu)
            * (multiplier_abs
                - ((P::ONES - lv.op2_sign_bit) * (lv.op2_value)
                    + (lv.op2_sign_bit) * (CpuState::<P>::shifted(32) - lv.op2_value))),
    );
    // Make sure multiplicand_abs is computed correctly from
    // op1_value.
    yield_constr.constraint(
        multiplicand_abs
            - ((P::ONES - lv.op1_sign_bit) * (lv.op1_value)
                + (lv.op1_sign_bit) * (CpuState::<P>::shifted(32) - lv.op1_value)),
    );
    // Make sure product_sign is either 0 or 1.
    yield_constr.constraint(lv.product_sign * (P::ONES - lv.product_sign));
    // For MUL/MULHU/SLL product sign should alwasy be 0.
    yield_constr
        .constraint((lv.inst.ops.sll + lv.inst.ops.mul + lv.inst.ops.mulhu) * (lv.product_sign));
    // If product_sign is 0 then res_when_prod_negative must be 0.
    yield_constr.constraint((P::ONES - lv.product_sign) * lv.res_when_prod_negative);
    // Make sure product_sign is computed correctly.
    yield_constr.constraint(
        lv.product_sign
            - ((lv.op1_sign_bit + lv.op2_sign_bit)
                - (P::Scalar::from_canonical_u32(2) * lv.op1_sign_bit * lv.op2_sign_bit)),
    );
    // Make sure product_low_bits_zero is computed correctly.
    yield_constr.constraint(
        lv.product_low_bits_zero - (P::ONES - lv.product_low_bits * lv.product_low_bits_inv),
    );
    // Make sure prodcut_zero is computed correctly.
    yield_constr.constraint(lv.product_zero - (P::ONES - product * lv.product_inv));
    // The following constraints are for SLL.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            lv.inst.ops.sll * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.input_b - op2));

        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(lv.inst.ops.sll * (multiplier_abs - lv.bitshift.multiplier));
    }

    // Make sure res_when_prod_negative is computed correctly.
    yield_constr.constraint(
        lv.product_sign
            * (lv.product_zero * (lv.res_when_prod_negative - high_limb)
                + (P::ONES - lv.product_zero)
                    * (lv.res_when_prod_negative
                        - (P::Scalar::from_noncanonical_u64(0xFFFF_FFFF) - high_limb
                            + lv.product_low_bits_zero))),
    );

    // Now, let's copy our results to the destination register:

    let destination = lv.dst_value;
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));

    // if product has negative sign:
    //   if product is zero:
    //     destination == 0 (which is same as high_limb)
    //   else
    //     destination == 2's complement of high_limb
    // else
    //   destination == high_limb
    //
    // NOTE: For MULHU it's always the case that product has positive sign.
    // And we assert that above in constraints for product_sign.
    yield_constr.constraint(
        (lv.inst.ops.mulh + lv.inst.ops.mulhsu + lv.inst.ops.mulhu)
            * (lv.product_sign * (destination - lv.res_when_prod_negative)
                + (P::ONES - lv.product_sign) * (destination - high_limb)),
    );

    // The constraints above would be enough, if our field was large enough.
    // However Goldilocks field is just a bit too small at order 2^64 - 2^32 + 1.
    //
    // Specifically, (1<<32) * (u32::MAX) === -1 (mod 2^64 - 2^32 + 1).
    // That means a high_limb of u32::MAX would behave like -1.  And hijinx ensues:
    //      let product = low_limb + base * high_limb;
    // would be equivalent to
    //      let product = low_limb - P::ONES;
    //
    // However, the largest combined result that an honest prover can produce is
    // u32::MAX * u32::MAX = 0xFFFF_FFFE_0000_0001.  So we can add a constraint
    // that high_limb is != 0xFFFF_FFFF == u32::MAX range.
    //
    // That curtails the exploit without invalidating any honest proofs.

    let diff = P::Scalar::from_noncanonical_u64(u32::MAX.into()) - lv.product_high_bits;
    yield_constr.constraint(
        (lv.inst.ops.mul + lv.inst.ops.mulhu + lv.inst.ops.sll)
            * (diff * lv.product_high_diff_inv - P::ONES),
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
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{standard_faster_config, ProveAndVerify, C, D, F};
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    #[test]
    fn prove_mulhsu_example() {
        type S = CpuStark<F, D>;
        let config = standard_faster_config();
        let a = -1_i32;
        let b = 4_294_967_295_u32;
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
        let (res, _overflow) = i64::from(a).overflowing_mul(i64::from(b));
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

        let proof = timed!(
            timing,
            "cpu proof",
            prove_table::<F, C, S, D>(stark, &config, trace_poly_values, [], &mut timing,)
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
