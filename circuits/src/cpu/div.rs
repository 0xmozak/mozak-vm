//! This module implements constraints for division operations, including
//! DIVU, REMU, and SRL instructions.
//!
//! Here, SRL stands for 'shift right logical'.  We can treat it as a variant of
//! unsigned multiplication.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;

/// Constraints for DIVU / REMU / SRL instructions
///
/// SRL stands for 'shift right logical'.  We can treat it as a variant of
/// unsigned division.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let dst = lv.dst_value;
    let ops = lv.inst.ops;

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that
    // > dividend = divisor × quotient + remainder.
    // In the following code, we are looking at p/q.
    let p = lv.op1_value;
    let q = lv.divisor;
    yield_constr.constraint((ops.divu + ops.remu) * (q - lv.op2_value));

    // Check: for DIVU and REMU, divisor `q` is assigned from `op2`
    yield_constr.constraint((lv.inst.ops.divu + lv.inst.ops.remu) * (q - lv.op2_value));

    // Check: for SRL, divisor `q` is assigned as `2^(op2 & 0b1_111)`.
    // We only take lowest 5 bits of the op2 for the shift amount.
    // This is following the RISC-V specification.
    // Bellow we use the And gadget to calculate the shift amount, and then use
    // Bitshift table to retrieve the corresponding power of 2, that we will assign
    // to the multiplier.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            ops.srl * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint(ops.srl * (and_gadget.input_b - op2));

        yield_constr.constraint(ops.srl * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(ops.srl * (q - lv.bitshift.multiplier));
    }

    // Check: the equation from the spec:
    //  p = q * m + r
    // (Interestingly, this holds even when q == 0.)
    let m = lv.quotient;
    let r = lv.remainder;
    yield_constr.constraint(m * q + r - p);

    // However, that constraint is not enough.
    // For example, a malicious prover could trivially fulfill it via
    //  m := 0, r := p

    // The solution is to constrain r further:
    //  0 <= r < q
    // (This only works when q != 0.)

    // Logically, these are two independent constraints:
    //      (A) 0 <= r
    //      (B) r < q
    // Part A is easy: we range-check r.
    // Part B is only slightly harder: borrowing the concept of 'slack variables' from linear programming (https://en.wikipedia.org/wiki/Slack_variable) we get:
    // (B') r + slack + 1 = q
    //      with range_check(slack)

    let slack = lv.remainder_slack;
    // Check: if divisor is not zero, then `r < q`.
    yield_constr.constraint(q * (r + slack + P::ONES - q));

    // Now we need to deal with division by zero.  The Risc-V spec says:
    //      p / 0 == 0xFFFF_FFFF
    //      p % 0 == p

    let q_inv = lv.divisor_inv;
    // Check: if divisor is zero, then m == u32::MAX following Risc-V spec.
    yield_constr.constraint(
        (P::ONES - q * q_inv) * (m - P::Scalar::from_noncanonical_u64(u32::MAX.into())),
    );
    // Check: if divisor is zero, then r == p following Risc-V spec.
    yield_constr.constraint((P::ONES - q * q_inv) * (r - p));

    // Finally, we select the correct output.

    // Check: for DIVU and SRL we output the quotient `m`.
    yield_constr.constraint((lv.inst.ops.divu + lv.inst.ops.srl) * (dst - m));
    // Check: for REMU we output the remainder `r`.
    yield_constr.constraint(lv.inst.ops.remu * (dst - r));
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{prop_assert_eq, ProptestConfig};
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{inv, ProveAndVerify, D, F};

    fn divu_remu_instructions(rd: u8) -> [Instruction; 2] {
        [
            Instruction {
                op: Op::DIVU,
                args: Args {
                    rd,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::REMU,
                args: Args {
                    rd,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
        ]
    }

    fn srl_instructions(rd: u8, q: u32) -> [Instruction; 2] {
        [
            Instruction {
                op: Op::SRL,
                args: Args {
                    rd,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SRL,
                args: Args {
                    rd,
                    rs1: 1,
                    imm: q,
                    ..Args::default()
                },
            },
        ]
    }

    fn prove_divu<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) -> Result<(), TestCaseError> {
        let (program, record) =
            simple_test_code(&divu_remu_instructions(rd), &[], &[(1, p), (2, q)]);
        prop_assert_eq!(
            record.executed[0].aux.dst_val,
            if let 0 = q { 0xffff_ffff } else { p / q }
        );
        prop_assert_eq!(
            record.executed[1].aux.dst_val,
            if let 0 = q { p } else { p % q }
        );
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }
    
    fn prove_srl<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(&srl_instructions(rd, q), &[], &[(1, p), (2, q)]);
        prop_assert_eq!(record.executed[0].aux.dst_val, p >> q);
        prop_assert_eq!(record.executed[1].aux.dst_val, p >> q);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn inv_is_big(x in u32_extra()) {
            type F = plonky2::field::goldilocks_field::GoldilocksField;
            let y = inv::<F>(u64::from(x));
            if x > 1 {
                prop_assert!(u64::from(u32::MAX) < y);
            }
        }

        #[test]
        fn prove_divu_cpu(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_divu::<CpuStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_srl_cpu(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            prove_srl::<CpuStark<F, D>>(p, q, rd)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]

        #[test]
        fn prove_divu_mozak(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_divu::<MozakStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_srl_mozak(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            prove_srl::<MozakStark<F, D>>(p, q, rd)?;
        }
    }
}
