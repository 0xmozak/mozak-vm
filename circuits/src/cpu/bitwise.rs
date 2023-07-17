//! This module implements the bitwise operations, AND, OR, and XOR.
//! We assume XOR is implemented directly as a cross-table lookup.
//! AND and OR are implemented as a combination of XOR and field element
//! arithmetic.
//!
//! We use two basic identities to implement AND, and OR:
//!  a | b = (a ^ b) + (a & b)
//!  a + b = (a ^ b) + 2 * (a & b)
//! The identities might seem a bit mysterious at first, but contemplating
//! a half-adder circuit should make them clear.
//!
//! Re-arranging and substituing yields:
//!  x & y := (x + y - (x ^ y)) / 2
//!  x | y := (x + y + (x ^ y)) / 2

use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_AND, COL_S_OR, COL_S_XOR,
    NUM_CPU_COLS, XOR_A, XOR_B, XOR_OUT,
};
use crate::utils::from_;

/// A struct to represent the output of binary operations
///
/// Especially AND, OR and XOR instructions.
#[derive(Debug, Clone)]
pub struct BinaryOp<P: PackedField> {
    pub input_a: P,
    pub input_b: P,
    pub output: P,
}

/// Re-usable gadget for AND constraints
/// Highest degree is one.
pub(crate) fn and_gadget<P: PackedField>(lv: &[P; NUM_CPU_COLS]) -> BinaryOp<P> {
    let input_a = lv[XOR_A];
    let input_b = lv[XOR_B];
    let xor_out = lv[XOR_OUT];
    let two: P::Scalar = from_(2_u32);
    BinaryOp {
        input_a,
        input_b,
        output: (input_a + input_b - xor_out) / two,
    }
}

/// Re-usable gadget for OR constraints
/// Highest degree is one.
pub(crate) fn or_gadget<P: PackedField>(lv: &[P; NUM_CPU_COLS]) -> BinaryOp<P> {
    let input_a = lv[XOR_A];
    let input_b = lv[XOR_B];
    let xor_out = lv[XOR_OUT];
    let two: P::Scalar = from_(2_u32);
    BinaryOp {
        input_a,
        input_b,
        output: (input_a + input_b + xor_out) / two,
    }
}

/// Re-usable gadget for XOR constraints
/// Highest degree is one.
pub(crate) fn xor_gadget<P: PackedField>(lv: &[P; NUM_CPU_COLS]) -> BinaryOp<P> {
    let input_a = lv[XOR_A];
    let input_b = lv[XOR_B];
    let output = lv[XOR_OUT];
    BinaryOp {
        input_a,
        input_b,
        output,
    }
}

/// Constraints to verify execution of AND, OR and XOR instructions.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let dst = lv[COL_DST_VALUE];

    for (selector, gadget) in [
        (lv[COL_S_AND], and_gadget(lv)),
        (lv[COL_S_OR], or_gadget(lv)),
        (lv[COL_S_XOR], xor_gadget(lv)),
    ] {
        yield_constr.constraint(selector * (gadget.input_a - op1));
        yield_constr.constraint(selector * (gadget.input_b - op2));
        yield_constr.constraint(selector * (gadget.output - dst));
    }
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
