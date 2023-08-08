use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuColumnsView;

/// Constraints for MUL / MULH / SLL instructions
///
/// SRL stands for 'shift left logical'.  We can treat it as a variant of
/// unsigned multiplication.
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: PRODUCT_LOW_BITS and PRODUCT_HIGH_BITS need range checking.

    let base = P::Scalar::from_noncanonical_u64(1 << 32);

    let multiplicand = lv.op1_value;
    let multiplier = lv.multiplier;
    let low_limb = lv.product_low_bits;
    let high_limb = lv.product_high_bits;
    let product = low_limb + base * high_limb;

    // The following constraints are for MUL, MULH and SLL op rows
    // The value should be the multiplication result
    yield_constr.constraint(
        (lv.inst.ops.mul + lv.inst.ops.mulhu + lv.inst.ops.sll)
            * (product - multiplicand * multiplier),
    );
    // The following constraints are for MUL and MULH op rows
    // Check: multiplier == op2_value
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.mulhu) * (multiplier - lv.op2_value));

    // The following constraints are for SLL op rows
    // `op2` contains the shift_by amount, represented by lower 5 bits of the value.
    // Hence we mask shift amount by `0x1F = 31` using `&` operation.
    // We then use the `bitshift` sub-table to calculate the shift multiplier.
    // Finally, we check that `multiplier` is indeed the value of the shift
    // multiplier.
    {
        // Check: output == input_a & input_b
        let and_gadget = and_gadget(&lv.xor);
        // Check: AND.input_a == 31
        yield_constr.constraint(
            lv.inst.ops.sll * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(31)),
        );
        let op2 = lv.op2_value;
        // Check: AND.input_b == op2
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.input_b - op2));

        // Check: AND.output == shift_amount
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.output - lv.bitshift.amount));
        // Check: multiplier == shift_multiplier
        yield_constr.constraint(lv.inst.ops.sll * (multiplier - lv.bitshift.multiplier));
    }

    // Now, let's copy our results to the destination register:

    let destination = lv.dst_value;
    // The following constraints are for MUL and SLL op rows
    // Check: result == product_low_bits
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));

    // The following constraints are for MULHU op rows
    // Check: result == product_high_bits (overflow part)
    yield_constr.constraint(lv.inst.ops.mulhu * (destination - high_limb));

    // The constraints above would be enough, if our field was large enough.
    // However Goldilocks field is just a bit too small at order 2^64 - 2^32 + 1.
    //
    // Specifically, (1<<32) * (u32::MAX) === -1 (mod 2^64 - 2^32 + 1).
    // Thus, when high_limb == u32::MAX:
    //       product = low_limb + base * high_limb = low_limb - P::ONES
    //
    // However, the largest combined result that an honest prover can produce is
    // u32::MAX * u32::MAX = 0xFFFF_FFFE_0000_0001.  So we can add a constraint
    // that high_limb is != 0xFFFF_FFFF == u32::MAX range.
    //
    // That curtails the exploit without invalidating any honest proofs.

    let diff = P::Scalar::from_noncanonical_u64(u32::MAX.into()) - lv.product_high_bits;
    // The following constraints are for MUL, MULHU and SLL op rows
    // Check:
    //  (u32::MAX - product_high_bits) * (u32::MAX - product_high_bits)^(-1) - 1 = 0
    // Equivalent to:
    //  u32::MAX != product_high_bits
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
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
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
            prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0x1F));
            prop_assert_eq!(record.executed[1].aux.dst_val, p << (q & 0x1F));
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
    }
}
