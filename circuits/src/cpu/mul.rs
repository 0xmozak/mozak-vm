//! This module implements constraints for multiplication operations, including
//! MUL, MULH and SLL instructions.
//!
//! Here, SLL stands for 'shift left logical'.  We can treat it as a variant of
//! unsigned multiplication.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // The Goldilocks field is carefully chosen to allow multiplication of u32
    // values without overflow, as max value is `u32::MAX^2=(2^32-1)^2`
    // And field size is `2^64-2^32+1`, which is `u32::MAX^2 + 2^32`
    let base = P::Scalar::from_noncanonical_u64(1 << 32);
    let ops = lv.inst.ops;

    let multiplicand = lv.op1_full_range();
    let multiplier = lv.multiplier;
    // TODO: range check this one.
    let low_limb = lv.product_low_bits;
    // TODO: range check the sign adjusted version of this one.
    // (Needs reoarginization later, because the adjustment is quadratic, with two sign bits.)
    let high_limb = lv.product_high_bits;
    let product = low_limb + base * high_limb;

    // Check: multiplication equation, `product == multiplicand * multiplier`.
    yield_constr.constraint(product - multiplicand * multiplier);
    // self.op2_value - self.op2_sign_bit * Self::shifted(32)

    // Check: for MUL and MULHU the multiplier is assigned the op2 value.
    yield_constr.constraint((ops.mul + ops.mulhu + ops.mulh + ops.mulhsu) * (multiplier - lv.op2_value));

    // Check: for SRL the multiplier is assigned as `2^(op2 & 0b1_111)`.
    // We only take lowest 5 bits of the op2 for the shift amount.
    // This is following the RISC-V specification.
    // Below we use the And gadget to calculate the shift amount, and then use
    // Bitshift table to retrieve the corresponding power of 2, that we will assign
    // to the multiplier.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            lv.inst.ops.sll * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.input_b - op2));

        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(lv.inst.ops.sll * (multiplier - lv.bitshift.multiplier));
    }

    // Check, that we select the correct output.

    let destination = lv.dst_value;
    // Check: For MUL and SLL, we assign the value of low limb as a result
    yield_constr.constraint((ops.mul + ops.sll) * (destination - low_limb));

    // + lv.op1_sign_bit * lv.op2_value + lv.op1_sign_bit * lv.op1_value
    // + lv.op1_sign_bit * lv.op2_sign_bit * base

    // Check: For MULHU, we assign the value of high limb as a result
    // yield_constr.constraint((ops.mulhu + ops.mulh + ops.mulhsu) * (destination - high_limb));

    // The constraints above would be enough, if our field was large enough.
    // However Goldilocks field is just a bit too small at order 2^64 - 2^32 + 1,
    // which allows for the following exploit to happen when high_limb == u32::MAX
    //
    // Specifically, (1<<32) * (u32::MAX) === -1 (mod 2^64 - 2^32 + 1).
    // Thus, when product high_limb == u32::MAX:
    //       product = low_limb + base * high_limb =
    //       = low_limb + (1<<32) * (u32::MAX) = low_limb - P::ONES
    //
    // Which means a malicious prover could evaluate some product in two different
    // ways, which is unacceptable.
    //
    // However, the largest combined result that an honest prover can produce is
    // u32::MAX * u32::MAX = 0xFFFF_FFFE_0000_0001.  So we can add a constraint
    // that high_limb is != 0xFFFF_FFFF == u32::MAX range to prevent such exploit.

    let diff = P::Scalar::from_noncanonical_u64(u32::MAX.into()) - lv.product_high_bits;
    // The following makes `product` deterministic as mentioned above by preventing
    // `high_limb` to take unreachable values.
    // Check: high limb != u32::MAX by checking that their diff is invertible.
    yield_constr.constraint(
        (ops.mul + ops.mulhu + ops.mulh + ops.mulhsu + lv.inst.ops.sll)
            * (diff * lv.product_high_diff_inv - P::ONES),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{i32_extra, reg, simple_test_code, u32_extra};
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::{prop_assert_eq, proptest};
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::cpu::stark::CpuStark;
    use crate::generation::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
    use crate::generation::program::generate_program_rom_trace;
    use crate::stark::mozak_stark::{PublicInputs, MozakStark};
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{standard_faster_config, ProveAndVerify, C, D, F};
    use crate::utils::from_u32;
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    #[test]
    fn prove_mulhsu_example() {
        type S = CpuStark<F, D>;
        let config = standard_faster_config();
        let a = -2_147_451_028_i32;
        let b = 2_147_483_648_u32;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::MULHSU,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a as u32), (7, b)],
        );
        let res = i64::from(a).wrapping_mul(i64::from(b));
        assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        let mut timing = TimingTree::new("mulhsu", log::Level::Debug);
        let cpu_trace = timed!(
            timing,
            "generate_cpu_trace",
            generate_cpu_trace(&program, &record)
        );
        let trace_poly_values = timed!(
            timing,
            "trace to poly",
            trace_to_poly_values(generate_cpu_trace_extended(
                cpu_trace,
                &generate_program_rom_trace(&program)
            ))
        );
        let stark = S::default();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let proof = timed!(
            timing,
            "cpu proof",
            prove_table::<F, C, S, D>(
                stark,
                &config,
                trace_poly_values,
                public_inputs.into(),
                &mut timing,
            )
        );
        let proof = proof.unwrap();
        let verification_res = timed!(
            timing,
            "cpu verification",
            verify_stark_proof(stark, proof, &config)
        );
        verification_res.unwrap();
        timing.print();
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_mul_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MUL,
                        args: Args {
                            rd: 8,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            let (low, _high) = a.widening_mul(b);
            prop_assert_eq!(record.executed[0].aux.dst_val, low);
            MozakStark::prove_and_verify(&program, &record).unwrap();
        }
        #[test]
        fn prove_mulhu_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MULHU,
                        args: Args {
                            rd: 9,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            let (_low, high) = a.widening_mul(b);
            prop_assert_eq!(record.executed[0].aux.dst_val, high);
            MozakStark::prove_and_verify(&program, &record).unwrap();
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulh_proptest(a in i32_extra(), b in i32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MULH,
                        args: Args {
                            rd: 8,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a as u32), (7, b as u32)],
            );
            let (res, overflow) = i64::from(a).overflowing_mul(i64::from(b));
            assert!(!overflow);
            prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
            MozakStark::prove_and_verify(&program, &record).unwrap();
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulhsu_proptest(a in i32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::MULHSU,
                        args: Args {
                            rd: 8,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a as u32), (7, b)],
            );
            let (res, _overflow) = i64::from(a).overflowing_mul(i64::from(b));
            prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
            MozakStark::prove_and_verify(&program, &record).unwrap();
        }

        #[test]
        fn prove_sll_proptest(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs1 != rd);
            prop_assume!(rs2 != rd);
            let (program, record) = simple_test_code(
                &[Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd,
                        rs1,
                        imm: q,
                        ..Args::default()
                    },
                }
                ],
                &[],
                &[(rs1, p), (rs2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0b1_1111));
            prop_assert_eq!(record.executed[1].aux.dst_val, p << (q & 0b1_1111));
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
    }
}
