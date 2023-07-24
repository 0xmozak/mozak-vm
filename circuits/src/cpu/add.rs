use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
    let added = lv.op1_value + lv.op2_value;
    let wrapped = added - wrap_at;

    yield_constr.constraint(lv.ops.add * (lv.dst_value - added) * (lv.dst_value - wrapped));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test, simple_test_code, u32_extra};

    use crate::test_utils::simple_proof_test;
    #[test]
    fn prove_add() {
        let record = simple_test(4, &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)], &[
            (6, 100),
            (7, 100),
        ]);
        assert_eq!(record.last_state.get_register_value(5), 100 + 100);
        simple_proof_test(&record.executed).unwrap();
    }
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_add_proptest(a in u32_extra(), b in u32_extra(), rd in 0_u8..32) {
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::ADD,
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
                if rd != 0 {
                    assert_eq!(record.executed[1].state.get_register_value(rd), a.wrapping_add(b));
                }
                simple_proof_test(&record.executed).unwrap();
            }
    }
}
