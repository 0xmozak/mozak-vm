//! This module implements the constraints for the ADD operation.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
    let added = lv.op1_value + lv.op2_value;
    let wrapped = added - wrap_at;

    // Check: the resulting sum is wrapped if necessary.
    // As the result is range checked, this make the choice deterministic,
    // even for a malicious prover.
    yield_constr.constraint(lv.inst.ops.add * (lv.dst_value - added) * (lv.dst_value - wrapped));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use anyhow::Result;
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};

    use crate::test_utils::{prove_with_stark, StarkType};

    fn prove_add_example(a: u32, b: u32, rd: u8, stark: StarkType) -> Result<()> {
        let (program, record) = simple_test_code(
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
        prove_with_stark(&program, &record, stark)
    }

    #[test]
    fn prove_add_mozak(){
        prove_add_example(100, 200, 5, StarkType::Mozak).unwrap();
    }
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_add_proptest(a in u32_extra(), b in u32_extra(), rd in 0_u8..32) {
                prove_add_example(a, b, rd, StarkType::Cpu).unwrap();
            }
    }
}
