use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_MUL, COL_S_MULHU, MUL_HIGH_BITS,
    MUL_HIGH_DIFF_INV, MUL_LOW_BITS, NUM_CPU_COLS,
};
use crate::utils::from_;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: MUL_LOW_BITS and MUL_HIGH_BITS need range checking.

    // The Goldilocks field is carefully chosen to allow multiplication of u32
    // values without overflow.
    let base = from_::<u64, P::Scalar>(1 << 32);

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE];
    let low_limb = lv[MUL_LOW_BITS];
    let high_limb = lv[MUL_HIGH_BITS];
    let product = low_limb + base * high_limb;

    yield_constr.constraint(product - op1 * op2);

    // Now, let's copy our results to the destination register:
    let is_mul = lv[COL_S_MUL];
    let is_mulhu = lv[COL_S_MULHU];

    let destination = lv[COL_DST_VALUE];
    yield_constr.constraint(is_mul * (destination - low_limb));
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

    let diff = from_::<u64, P::Scalar>(u32::MAX.into()) - lv[MUL_HIGH_BITS];
    yield_constr.constraint(diff * lv[MUL_HIGH_DIFF_INV] - P::ONES);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

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
            assert_eq!(record.executed[1].state.get_register_value(8), low);
            assert_eq!(record.executed[2].state.get_register_value(9), high);
            simple_proof_test(&record.executed).unwrap();
        }
    }
}
