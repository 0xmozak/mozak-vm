use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // We do not need to constrain memory address because it will be range checked.
    yield_constr.constraint(lv.ops.sb * (lv.dst_value - lv.op2_value));
    // TODO: support for SH / SW
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;

    // TODO: use memory_trace_test_case when other ops supported.
    //
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_sb_proptest(a in u32_extra(), b in u32_extra()) {
            let record = simple_test_code(
                &[
                    Instruction {
                        op: Op::SB,
                        args: Args {
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                    Instruction {
                        op: Op::LBU,
                        args: Args {
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );

            CpuStark::prove_and_verify(&record.executed).unwrap();
        }
    }
}
