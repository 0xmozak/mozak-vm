use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{COL_DST_VALUE, COL_SLTU_CHECK, COL_S_SLTU, NUM_CPU_COLS};
use super::utils::pc_ticks_up;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // lv[COL_SLTU_CHECK] is either 0 or 1
    // NOTE: We have to relax this check if COL_SLTU_CHECK is used as auxlilary data
    // column in future.
    yield_constr.constraint(lv[COL_SLTU_CHECK] * (lv[COL_SLTU_CHECK] - P::ONES));
    yield_constr.constraint(
        lv[COL_S_SLTU]
            * (lv[COL_SLTU_CHECK] * (lv[COL_DST_VALUE] - P::ONES)
                + ((P::ONES - lv[COL_SLTU_CHECK]) * (lv[COL_DST_VALUE]))),
    );

    yield_constr.constraint_transition((lv[COL_S_SLTU]) * pc_ticks_up(lv, nv));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::any;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
            #[test]
            fn prove_sltu_proptest(a in any::<u32>(), b in any::<u32>()) {
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::SLTU,
                        args: Args {
                            rd: 5,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    }],
                    &[],
                    &[(6, a), (7, b)],
                );
                assert_eq!(record.last_state.get_register_value(5), u32::from(a < b));
                simple_proof_test(&record.executed).unwrap();
            }
            #[test]
            fn prove_sltiu_proptest(a in any::<u32>(), b in any::<u32>()) {
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::SLTU,
                        args: Args {
                            rd: 5,
                            rs1: 6,
                            rs2: 0,
                            imm: b,
                        },
                    }],
                    &[],
                    &[(6, a)],
                );
                assert_eq!(record.last_state.get_register_value(5), u32::from(a < b));
                simple_proof_test(&record.executed).unwrap();
            }
    }
}
