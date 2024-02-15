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

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::columns::CpuState;
use crate::xor::columns::XorView;

/// A struct to represent the output of binary operations
///
/// Implemented for AND, OR and XOR instructions.
#[derive(Debug, Clone)]
pub struct BinaryOp<P: PackedField> {
    pub input_a: P,
    pub input_b: P,
    pub output: P,
}

pub struct BinaryOpExtensionTarget<const D: usize> {
    pub input_a: ExtensionTarget<D>,
    pub input_b: ExtensionTarget<D>,
    pub output: ExtensionTarget<D>,
}

/// Re-usable gadget for AND constraints.
/// It has access to already constrained XOR evaluation and based on that
/// constrains the AND evaluation: `x & y := (x + y - xor(x,y)) / 2`
/// This gadget can be used to anywhere in the constraint system.
pub(crate) fn and_gadget<P: PackedField>(xor: &XorView<P>) -> BinaryOp<P> {
    let two = P::Scalar::from_noncanonical_u64(2);
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        output: (xor.a + xor.b - xor.out) / two,
    }
}

pub(crate) fn and_gadget_extension_targets<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    xor: &XorView<ExtensionTarget<D>>,
) -> BinaryOpExtensionTarget<D> {
    let two = F::Extension::from_canonical_u64(2);
    let two_inv = builder.constant_extension(two.inverse());
    let a_add_b = builder.add_extension(xor.a, xor.b);
    let a_add_b_sub_xor = builder.sub_extension(a_add_b, xor.out);
    BinaryOpExtensionTarget {
        input_a: xor.a,
        input_b: xor.b,
        output: builder.mul_extension(a_add_b_sub_xor, two_inv),
    }
}

/// Re-usable gadget for OR constraints
/// It has access to already constrained XOR evaluation and based on that
/// constrains the OR evaluation: `x | y := (x + y + xor(x,y)) / 2`
/// This gadget can be used to anywhere in the constraint system.
pub(crate) fn or_gadget<P: PackedField>(xor: &XorView<P>) -> BinaryOp<P> {
    let two = P::Scalar::from_noncanonical_u64(2);
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        output: (xor.a + xor.b + xor.out) / two,
    }
}

pub(crate) fn or_gadget_extension_targets<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    xor: &XorView<ExtensionTarget<D>>,
) -> BinaryOpExtensionTarget<D> {
    let two = F::Extension::from_canonical_u64(2);
    let two_inv = builder.constant_extension(two.inverse());
    let a_add_b = builder.add_extension(xor.a, xor.b);
    let a_add_b_add_xor = builder.add_extension(a_add_b, xor.out);
    BinaryOpExtensionTarget {
        input_a: xor.a,
        input_b: xor.b,
        output: builder.mul_extension(a_add_b_add_xor, two_inv),
    }
}

/// Re-usable gadget for XOR constraints
/// Constrains that the already constrained underlying XOR evaluation has been
/// done on the same inputs and produced the same output as this gadget.
/// This gadget can be used to anywhere in the constraint system.
pub(crate) fn xor_gadget<P: PackedField>(xor: &XorView<P>) -> BinaryOp<P> {
    BinaryOp {
        input_a: xor.a,
        input_b: xor.b,
        output: xor.out,
    }
}

pub(crate) fn xor_gadget_extension_targets<const D: usize>(
    xor: &XorView<ExtensionTarget<D>>,
) -> BinaryOpExtensionTarget<D> {
    BinaryOpExtensionTarget {
        input_a: xor.a,
        input_b: xor.b,
        output: xor.out,
    }
}

/// Constraints for the AND, OR and XOR opcodes.
/// As each opcode has an associated selector, we use selectors to enable only
/// the correct opcode constraints. It can be that all selectors are not active,
/// representing that the operation is neither AND, nor OR or XOR.
/// The operation constraints are maintained in the corresponding gadget, and we
/// just need to make sure the gadget gets assigned correct inputs and output.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let op1 = lv.op1_value;
    let op2 = lv.op2_value;
    let dst = lv.dst_value;

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

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let op1 = lv.op1_value;
    let op2 = lv.op2_value;
    let dst = lv.dst_value;

    for (selector, gadget) in [
        (
            lv.inst.ops.and,
            and_gadget_extension_targets(builder, &lv.xor),
        ),
        (
            lv.inst.ops.or,
            or_gadget_extension_targets(builder, &lv.xor),
        ),
        (lv.inst.ops.xor, xor_gadget_extension_targets(&lv.xor)),
    ] {
        let input_a = builder.sub_extension(gadget.input_a, op1);
        let input_b = builder.sub_extension(gadget.input_b, op2);
        let output = builder.sub_extension(gadget.output, dst);
        let constr = builder.mul_extension(selector, input_a);
        yield_constr.constraint(builder, constr);
        let constr = builder.mul_extension(selector, input_b);
        yield_constr.constraint(builder, constr);
        let constr = builder.mul_extension(selector, output);
        yield_constr.constraint(builder, constr);
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::u32_extra;
    use mozak_runner::util::execute_code;
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

        let (program, record) = execute_code(code, &[], &[(6, a), (7, b)]);
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
