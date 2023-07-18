use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_MUL, COL_S_MULHU, COL_S_SLL,
    MULTIPLIER, NUM_CPU_COLS, POWERS_OF_2_IN, POWERS_OF_2_OUT, PRODUCT_HIGH_BITS,
    PRODUCT_HIGH_DIFF_INV, PRODUCT_LOW_BITS,
};
use crate::utils::from_;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: PRODUCT_LOW_BITS and PRODUCT_HIGH_BITS need range checking.

    let is_mul = lv[COL_S_MUL];
    let is_mulhu = lv[COL_S_MULHU];
    let is_sll = lv[COL_S_SLL];
    // The Goldilocks field is carefully chosen to allow multiplication of u32
    // values without overflow.
    let base = from_::<u64, P::Scalar>(1 << 32);

    let multiplicand = lv[COL_OP1_VALUE];
    let multiplier = lv[MULTIPLIER];
    let low_limb = lv[PRODUCT_LOW_BITS];
    let high_limb = lv[PRODUCT_HIGH_BITS];
    let product = low_limb + base * high_limb;

    yield_constr.constraint((is_mul + is_mulhu + is_sll) * (product - multiplicand * multiplier));
    yield_constr.constraint((is_mul + is_mulhu) * (multiplier - lv[COL_OP2_VALUE]));
    // The following constraints are for SLL.
    {
        let and_gadget = and_gadget(lv);
        yield_constr.constraint(is_sll * (and_gadget.input_a - from_::<u8, P::Scalar>(0x1F)));
        let op2 = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
        yield_constr.constraint(is_sll * (and_gadget.input_b - op2));

        yield_constr.constraint(is_sll * (and_gadget.output - lv[POWERS_OF_2_IN]));
        yield_constr.constraint(is_sll * (multiplier - lv[POWERS_OF_2_OUT]));
    }

    // Now, let's copy our results to the destination register:

    let destination = lv[COL_DST_VALUE];
    yield_constr.constraint((is_mul + is_sll) * (destination - low_limb));
    yield_constr.constraint(is_mulhu * (destination - high_limb));

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

    let diff = from_::<u64, P::Scalar>(u32::MAX.into()) - lv[PRODUCT_HIGH_BITS];
    yield_constr
        .constraint((is_mul + is_mulhu + is_sll) * (diff * lv[PRODUCT_HIGH_DIFF_INV] - P::ONES));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{reg, simple_test_code, u32_extra};
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::{prop_assert_eq, proptest};

    use crate::test_utils::simple_proof_test;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_mul_proptest(a in u32_extra(), b in u32_extra()) {
            let record = simple_test_code(
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
            simple_proof_test(&record.executed).unwrap();
        }

        #[test]
        fn prove_sll_proptest(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs1 != rd);
            prop_assume!(rs2 != rd);
            let record = simple_test_code(
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
                        rs2: 0,
                        imm: q,
                    },
                }
                ],
                &[],
                &[(rs1, p), (rs2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0x1F));
            prop_assert_eq!(record.executed[1].aux.dst_val, p << (q & 0x1F));
            simple_proof_test(&record.executed).unwrap();
        }
    }
}
