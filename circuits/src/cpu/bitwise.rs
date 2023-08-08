//! This module implements the bitwise operations, AND, OR, and XOR.
//! We assume XOR is implemented directly as a cross-table lookup.
//! AND and OR are implemented as a combination of XOR and field element
//! arithmetic.
//!
//! Note: we are already assuming that all the values are either 0 or 1.
//! This check is enforced by the [`XorView`] Sub-Table
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
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;
use crate::bitwise::columns::XorView;

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
/// Converts Xor output to And output.
/// Highest degree is one.
/// x & y := (x + y - (x ^ y)) / 2
pub(crate) fn and_gadget<P: PackedField>(xor: &XorView<P>) -> BinaryOp<P> {
    let two = P::Scalar::from_noncanonical_u64(2);
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        output: (xor.a + xor.b - xor.out) / two,
    }
}

/// Re-usable gadget for OR constraints
/// Converts Xor output to Or output.
/// Highest degree is one.
/// x | y := (x + y + (x ^ y)) / 2
pub(crate) fn or_gadget<P: PackedField>(xor: &XorView<P>) -> BinaryOp<P> {
    let two = P::Scalar::from_noncanonical_u64(2);
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        output: (xor.a + xor.b + xor.out) / two,
    }
}

/// Re-usable gadget for XOR constraints
/// Wraps Xor output.
/// Highest degree is one.
/// x ^ y := x ^ y
pub(crate) fn xor_gadget<P: PackedField>(xor: &XorView<P>) -> BinaryOp<P> {
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        output: xor.out,
    }
}

/// Constraints to verify execution of AND, OR and XOR ops.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let op1 = lv.op1_value;
    let op2 = lv.op2_value;
    let dst = lv.dst_value;
    
    // For each of the And, Or, and Xor gadgets assign them with correct values.
    // The underlying gadgets use the [`bitwise.BitwiseStark`] Xor Stark and 
    // convert its results to the desired output.
    
    // Check: inputs and output of Bit Gadgets have been assigned correctly
    for (selector, gadget) in [
        (lv.inst.ops.and, and_gadget(&lv.xor)),
        (lv.inst.ops.or, or_gadget(&lv.xor)),
        (lv.inst.ops.xor, xor_gadget(&lv.xor)),
    ] {
        yield_constr.constraint(selector * (gadget.input_a - op1));
        yield_constr.constraint(selector * (gadget.input_b - op2));
        yield_constr.constraint(selector * (gadget.output - dst));
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::bitwise::stark::BitwiseStark;
    use crate::test_utils::ProveAndVerify;

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
                    ..Args::default()
                },
            })
            .collect();

            let (program, record) = simple_test_code(&code, &[], &[(6, a), (7, b)]);
            BitwiseStark::prove_and_verify(&program, &record.executed).unwrap();
        }
    }
}
