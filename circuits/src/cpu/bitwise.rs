use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    AND_A, AND_B, AND_OUT, COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_AND,
    COL_S_OR, COL_S_XOR, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

/// Constraints to verify execution of AND, OR and XOR instructions.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_and = lv[COL_S_AND];
    let is_or = lv[COL_S_OR];
    let is_xor = lv[COL_S_XOR];

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let dst = lv[COL_DST_VALUE];

    let and_a = lv[AND_A];
    let and_b = lv[AND_B];
    let and_out = lv[AND_OUT];

    yield_constr.constraint((is_and + is_xor) * (op1 - and_a));
    yield_constr.constraint((is_and + is_xor) * (op2 - and_b));

    yield_constr.constraint(is_and * (and_out - dst));
    // We implement XOR in terms of AND:
    // x ^ y == x + y - 2 * (x & y)
    yield_constr.constraint(is_xor * (op1 + op2 - column_of_xs::<P>(2) * and_out - dst));

    let u32_max: P = column_of_xs(u32::MAX.into());

    // We implement OR in terms of AND thanks to De Morgan's law:
    // a | b == !(!a & !b)
    // with !a == u32::max - a
    yield_constr.constraint(is_or * (u32_max - op1 - and_a));
    yield_constr.constraint(is_or * (u32_max - op2 - and_b));
    yield_constr.constraint(is_or * (u32_max - dst - and_out));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::{any, prop_oneof, Just, ProptestConfig};
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;

    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_and_proptest(
                a in any::<u32>(),
                b in any::<u32>(),
                rd in 0_u8..32,
                kind in prop_oneof![Just(Op::AND), Just(Op::OR), Just(Op::XOR)])
            {
                let record = simple_test_code(
                    &[Instruction {
                        op: kind,
                        args: Args {
                            rd,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    }],
                    &[],
                    &[(6, a), (7, b)],
                );
                simple_proof_test(&record.executed).unwrap();
            }
    }
}
