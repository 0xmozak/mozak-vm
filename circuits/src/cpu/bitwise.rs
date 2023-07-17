use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_AND, COL_S_OR, COL_S_XOR,
    NUM_CPU_COLS, XOR_A, XOR_B, XOR_OUT,
};
use crate::utils::from_;

/// Constraints to verify execution of AND, OR and XOR instructions.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // We use two basic identities to implement AND, and OR in terms of:
    //  a | b = (a ^ b) + (a & b)
    //  a + b = (a ^ b) + 2 * (a & b)
    // The identities might seem a bit mysterious at first, but contemplating
    // a half-adder circuit should make them clear.
    //
    // Re-arranging and substituing yields:
    //  2 * (x & y) = x + y - (x ^ y)
    //  2 * (x | y) = x + y + (x ^ y)
    let is_and = lv[COL_S_AND];
    let is_or = lv[COL_S_OR];
    let is_xor = lv[COL_S_XOR];

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let dst = lv[COL_DST_VALUE];

    let xor_a = lv[XOR_A];
    let xor_b = lv[XOR_B];
    let xor_out = lv[XOR_OUT];

    yield_constr.constraint(xor_a - op1);
    yield_constr.constraint(xor_b - op2);

    yield_constr.constraint(is_xor * (xor_out - dst));
    let two: P::Scalar = from_(2_u32);
    yield_constr.constraint(is_and * (op1 + op2 - xor_out - dst * two));
    yield_constr.constraint(is_or * (op1 + op2 + xor_out - dst * two));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_bitwise_proptest(
            a in u32_extra(),
            b in u32_extra(),
            imm in u32_extra(),
            use_imm in any::<bool>())
        {
            let (b, imm) = if use_imm {
                (0, imm)
            } else {
                (b, 0)
            };
            let code: Vec<_> = [Op::AND, Op::OR, Op::XOR]
            .into_iter()
            .map(|kind| Instruction {
                op: kind,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    imm,
                },
            })
            .collect();

            let record = simple_test_code(&code, &[], &[(6, a), (7, b)]);
            simple_proof_test(&record.executed).unwrap();
        }
    }
}
