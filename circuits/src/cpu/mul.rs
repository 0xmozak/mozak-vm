//! This module implements the multiplications operation constraints, including
//! MUL, MULH and SLL instructions.
//!
//! Here, SLL stands for 'shift left logical'.  We can treat it as a variant of
//! unsigned multiplication.

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
    // values almost without overflow.
    // We tackle the edge case at the end of our constraints.
    let base = P::Scalar::from_noncanonical_u64(1 << 32);

    let multiplicand = lv.op1_value;
    let multiplier = lv.multiplier;
    let low_limb = lv.product_low_bits;
    let high_limb = lv.product_high_bits;
    let product = low_limb + base * high_limb;

    // Check: multiplication equation, `product == multiplicand * multiplier`.
    // (Not accounting for overflows for now).
    yield_constr.constraint(product - multiplicand * multiplier);

    // Check: for MUL and MULHU the multiplier is assigned the op2 value.
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.mulhu) * (multiplier - lv.op2_value));

    // Check: for SRL the multiplier is assigned as `2^(op2 & 0b1_111)`.
    // We only take lowest 5 bits of the op2 for the shift amount.
    // This is following the RISC-V specification.
    // Bellow we use the And gadget to calculate the shift amount, and then use
    // Bitshift table to retrieve the corresponding power of 2, that we will assign
    // to the multiplier.
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

    // Check, that we select the correct output.

    let destination = lv.dst_value;
    // Check: For MUL and SLL, we assign the value of low limb as a result
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));

    // Check: For MULHU, we assign the value of high limb as a result
    yield_constr.constraint(lv.inst.ops.mulhu * (destination - high_limb));

    // The constraints above would be enough, if our field was large enough.
    // However Goldilocks field is just a bit too small at order 2^64 - 2^32 + 1.
    //
    // Specifically, (1<<32) * (u32::MAX) === -1 (mod 2^64 - 2^32 + 1).
    // Thus, when product high_limb == u32::MAX:
    //       product = low_limb + base * high_limb = low_limb - P::ONES
    //
    // However, the largest combined result that an honest prover can produce is
    // u32::MAX * u32::MAX = 0xFFFF_FFFE_0000_0001.  So we can add a constraint
    // that high_limb is != 0xFFFF_FFFF == u32::MAX range.
    //
    // That curtails the exploit without invalidating any honest proofs.

    let diff = P::Scalar::from_noncanonical_u64(u32::MAX.into()) - lv.product_high_bits;
    // The following constraints `product` as mentioned above in cases of overflows.
    // Check: high limb != u32::MAX by checking that diff is invertible.
    yield_constr.constraint(
        (lv.inst.ops.mul + lv.inst.ops.mulhu + lv.inst.ops.sll)
            * (diff * lv.product_high_diff_inv - P::ONES),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{reg, simple_test_code, u32_extra};
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::{prop_assert_eq, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;
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
