use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_ADD, NUM_CPU_COLS,
};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
    let added = lv[COL_OP1_VALUE] + lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let wrapped = added - wrap_at;

    yield_constr
        .constraint(lv[COL_S_ADD] * (lv[COL_DST_VALUE] - added) * (lv[COL_DST_VALUE] - wrapped));
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
