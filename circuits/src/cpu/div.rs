use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DIVU_M, COL_DIVU_R, COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_DIVU,
    NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p = lv[COL_OP1_VALUE];
    let q = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    // m needs a range-check.
    let m = lv[COL_DIVU_M];
    let r = lv[COL_DIVU_R];

    yield_constr.constraint(m * q + r - p);
    // range check:
    // 0 =< m =< u32::MAX
    // 0 =< r =< u32::MAX

    // 0 =< r < p

    // p/q = m Remainder r
    // We have: m * q + r = p
    //

    // let wrap_at: P = column_of_xs(1 << 32);
    // let added = lv[COL_OP1_VALUE] + lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    // let wrapped = added - wrap_at;

    // yield_constr
    //     .constraint(lv[COL_S_ADD] * (lv[COL_DST_VALUE] - added) *
    // (lv[COL_DST_VALUE] - wrapped));
}

#[cfg(test)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::{any, ProptestConfig};
    use proptest::{prop_assert, proptest};

    use crate::test_utils::{inv, simple_proof_test};
    proptest! {
        #[test]
        fn inv_is_big(x in any::<u32>()) {
            type F = plonky2::field::goldilocks_field::GoldilocksField;
            let y = inv::<F>(u64::from(x));
            if x != 0 {
                prop_assert!(u64::from(u32::MAX) < y);
            }
        }
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn prove_divu_proptest(a in any::<u32>(), b in any::<u32>(), rd in 0_u8..32) {
            use crate::test_utils::simple_proof_test;
            let record = simple_test_code(
                &[Instruction {
                    op: Op::DIVU,
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
            // if rd != 0 {
            //     assert_eq!(record.executed[1].state.get_register_value(rd), a.wrapping_add(b));
            // }
            simple_proof_test(&record.executed).unwrap();
        }
    }
}
