//! This module implements constraints for bitwise operations, AND, OR, and XOR.
//! We assume XOR is implemented directly as a cross-table lookup.
//! AND and OR are implemented as a combination of XOR and field element
//! arithmetic.
//!
//!
//! We use two basic identities to implement AND, and OR:
//! `
//!  a | b = (a ^ b) + (a & b)
//!  a + b = (a ^ b) + 2 * (a & b)
//! `
//! The identities might seem a bit mysterious at first, but contemplating
//! a half-adder circuit should make them clear.
//! Note that these identities work for any `u32` numbers `a` and `b`.
//!
//! Re-arranging and substituting yields:
//! `
//!  x & y := (x + y - (x ^ y)) / 2
//!  x | y := (x + y + (x ^ y)) / 2
//! `
//! Or, without division:
//! `
//!  2 * (x & y) := (x + y - (x ^ y))
//!  2 * (x | y) := (x + y + (x ^ y))
//! `

use expr::Expr;

use super::columns::CpuState;
use crate::expr::ConstraintBuilder;
use crate::xor::columns::XorView;

/// A struct to represent the output of binary operations
///
/// Implemented for AND, OR and XOR instructions.
#[derive(Debug, Clone)]
pub struct BinaryOp<P> {
    pub input_a: P,
    pub input_b: P,
    /// Our constraints naturally give us the doubled output; currently our
    /// `Expr` mechanism doesn't support multiplicative inverses of constants,
    /// so we work with doubled output, and just also double the other side of
    /// our equations.
    ///
    /// If necessary, we can fix the `Expr` mechanism later, but there's no
    /// hurry.  (Do keep in mind that `PackedField` does not support
    /// multiplicative inverses.)
    pub doubled_output: P,
}

/// Re-usable gadget for AND constraints.
/// It has access to already constrained XOR evaluation and based on that
/// constrains the AND evaluation: `2 * (x & y) := x + y - xor(x,y)`
/// This gadget can be used to anywhere in the constraint system.
pub(crate) fn and_gadget<'a, P: Copy>(xor: &XorView<Expr<'a, P>>) -> BinaryOp<Expr<'a, P>> {
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        doubled_output: xor.a + xor.b - xor.out,
    }
}

/// Re-usable gadget for OR constraints
/// It has access to already constrained XOR evaluation and based on that
/// constrains the OR evaluation: `2 * (x | y) := x + y + xor(x,y)`
/// This gadget can be used to anywhere in the constraint system.
pub(crate) fn or_gadget<'a, P: Copy>(xor: &XorView<Expr<'a, P>>) -> BinaryOp<Expr<'a, P>> {
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        doubled_output: xor.a + xor.b + xor.out,
    }
}

/// Re-usable gadget for XOR constraints
/// Constrains that the already constrained underlying XOR evaluation has been
/// done on the same inputs and produced the same output as this gadget.
/// This gadget can be used to anywhere in the constraint system.
pub(crate) fn xor_gadget<'a, P: Copy>(xor: &XorView<Expr<'a, P>>) -> BinaryOp<Expr<'a, P>> {
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        doubled_output: 2 * xor.out,
    }
}

/// Constraints for the AND, OR and XOR opcodes.
/// As each opcode has an associated selector, we use selectors to enable only
/// the correct opcode constraints. It can be that all selectors are not active,
/// representing that the operation is neither AND, nor OR or XOR.
/// The operation constraints are maintained in the corresponding gadget, and we
/// just need to make sure the gadget gets assigned correct inputs and output.
pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let op1 = lv.op1_value;
    let op2 = lv.op2_value;
    let dst = lv.dst_value;

    for (selector, gadget) in [
        (lv.inst.ops.and, and_gadget(&lv.xor)),
        (lv.inst.ops.or, or_gadget(&lv.xor)),
        (lv.inst.ops.xor, xor_gadget(&lv.xor)),
    ] {
        cb.always(selector * (gadget.input_a - op1));
        cb.always(selector * (gadget.input_b - op2));
        cb.always(selector * (gadget.doubled_output - 2 * dst));
    }
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::u32_extra;
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};
    use crate::xor::stark::XorStark;

    fn prove_bitwise<Stark: ProveAndVerify>(a: u32, b: u32, imm: u32, use_imm: bool) {
        let (b, imm) = if use_imm { (0, imm) } else { (b, 0) };
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

        let (program, record) = code::execute(code, &[], &[(6, a), (7, b)]);
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_bitwise_xor(
            a in u32_extra(),
            b in u32_extra(),
            imm in u32_extra(),
            use_imm in any::<bool>())
        {
           prove_bitwise::<XorStark<F, D>>(a, b, imm, use_imm);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_bitwise_mozak(
            a in u32_extra(),
            b in u32_extra(),
            imm in u32_extra(),
            use_imm in any::<bool>())
        {
           prove_bitwise::<MozakStark<F, D>>(a, b, imm, use_imm);
        }
    }
}
