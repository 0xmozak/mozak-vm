use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: PRODUCT_LOW_BITS and PRODUCT_HIGH_BITS need range checking.

    // The Goldilocks field is carefully chosen to allow multiplication of u32
    // values without overflow.
    let base = P::Scalar::from_noncanonical_u64(1 << 32);

    let multiplicand = lv.op1_full_range();
    let multiplier = lv.multiplier;
    let low_limb = lv.product_low_bits;
    let high_limb = lv.product_high_bits;
    let product = low_limb + base * high_limb;
    let expected_product = multiplicand * multiplier;

    yield_constr.constraint(
        (lv.inst.ops.mul + lv.inst.ops.mulhu + lv.inst.ops.sll + lv.inst.ops.mulh)
            * (product - expected_product),
    );

    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.mulhu) * (multiplier - lv.op2_value));
    // The following constraints are for SLL.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            lv.inst.ops.sll * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.input_b - op2));

        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(lv.inst.ops.sll * (multiplier - lv.bitshift.multiplier));
    }

    // Now, let's copy our results to the destination register:

    let destination = lv.dst_value;
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));
    yield_constr.constraint((lv.inst.ops.mulhu) * (destination - high_limb));
    yield_constr.constraint((lv.inst.ops.mulh) * (destination - high_limb));

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
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::{prop_assert_eq, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;
    #[test]
    fn prove_mul_example() {
        let b = 4_294_967_295;
        let a = 2_147_502_408;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::MUL,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a), (7, b)],
        );
        let (low, _high) = a.widening_mul(b);
        assert_eq!(record.executed[0].aux.dst_val, low);
        CpuStark::prove_and_verify(&program, &record).unwrap();
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    #[test]
    fn prove_mulh_example() {
        // let a = -1_i32;
        // let b = 1;
        let a = 1;
        let b = 1;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::MULH,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a as u32), (7, b as u32)],
        );
        let (res, overflow) = i64::from(a).overflowing_mul(i64::from(b));
        assert!(!overflow);
        assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        CpuStark::prove_and_verify(&program, &record).unwrap();
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
            let (low, high) = a.widening_mul(b);
            prop_assert_eq!(record.executed[0].aux.dst_val, low);
            prop_assert_eq!(record.executed[1].aux.dst_val, high);
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
