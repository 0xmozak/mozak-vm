//! This module implements constraints for shift operations, including
//! SRL,SRA and SLL instructions.
//!
//! Here, SLL stands for 'shift left logical'.  We can treat it as a variant of
//! unsigned multiplication. Same for SRL and SRA, but with division.

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::bitwise::{and_gadget, and_gadget_extension_targets};
use super::columns::{rs2_value_extension_target, CpuState};

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_shift = lv.inst.ops.sll + lv.inst.ops.srl + lv.inst.ops.sra;
    // Check: multiplier is assigned as `2^(rs2 value & 0b1_111)`.
    // We only take lowest 5 bits of the rs2 for the shift amount.
    // This is following the RISC-V specification.
    // Below we use the And gadget to calculate the shift amount, and then use
    // Bitshift table to retrieve the corresponding power of 2, that we will assign
    // to the multiplier.
    let and_gadget = and_gadget(&lv.xor);
    yield_constr
        .constraint(is_shift * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)));
    yield_constr.constraint(is_shift * (and_gadget.input_b - lv.rs2_value() - lv.inst.imm_value));

    yield_constr.constraint(is_shift * (and_gadget.output - lv.bitshift.amount));
}

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let sll_add_srl = builder.add_extension(lv.inst.ops.sll, lv.inst.ops.srl);
    let is_shift = builder.add_extension(sll_add_srl, lv.inst.ops.sra);

    let and_gadget = and_gadget_extension_targets(builder, &lv.xor);

    let mask = builder.constant_extension(F::Extension::from_canonical_u64(0b1_1111));
    let input_a_sub_mask = builder.sub_extension(and_gadget.input_a, mask);
    let shift_constr = builder.mul_extension(is_shift, input_a_sub_mask);
    yield_constr.constraint(builder, shift_constr);

    let rs2_value = rs2_value_extension_target(builder, lv);
    let rs2_value_imm = builder.add_extension(rs2_value, lv.inst.imm_value);
    let input_b_sub_rs2_imm = builder.sub_extension(and_gadget.input_b, rs2_value_imm);
    let rs2_constr = builder.mul_extension(is_shift, input_b_sub_rs2_imm);
    yield_constr.constraint(builder, rs2_constr);

    let output_sub_amount = builder.sub_extension(and_gadget.output, lv.bitshift.amount);
    let output_constr = builder.mul_extension(is_shift, output_sub_amount);
    yield_constr.constraint(builder, output_constr);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{reg, u32_extra};
    use mozak_runner::util::execute_code;
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert_eq, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_srl<Stark: ProveAndVerify>(
        p: u32,
        q: u32,
        rs1: u8,
        rs2: u8,
        rd: u8,
    ) -> Result<(), TestCaseError> {
        prop_assume!(rs1 != rs2);
        prop_assume!(rs1 != rd);
        prop_assume!(rs2 != rd);
        let (program, record) = execute_code(
            [
                Instruction {
                    op: Op::SRL,
                    args: Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRL,
                    args: Args {
                        rd,
                        rs1,
                        imm: q,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(rs1, p), (rs2, q)],
        );
        prop_assert_eq!(record.executed[0].aux.dst_val, p >> (q & 0b1_1111));
        prop_assert_eq!(record.executed[1].aux.dst_val, p >> (q & 0b1_1111));
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }
    fn prove_sll<Stark: ProveAndVerify>(
        p: u32,
        q: u32,
        rs1: u8,
        rs2: u8,
        rd: u8,
    ) -> Result<(), TestCaseError> {
        prop_assume!(rs1 != rs2);
        prop_assume!(rs1 != rd);
        prop_assume!(rs2 != rd);
        let (program, record) = execute_code(
            [
                Instruction {
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
                },
            ],
            &[],
            &[(rs1, p), (rs2, q)],
        );
        prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0b1_1111));
        prop_assert_eq!(record.executed[1].aux.dst_val, p << (q & 0b1_1111));
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }
    fn prove_sra<Stark: ProveAndVerify>(
        p: u32,
        q: u32,
        rs1: u8,
        rs2: u8,
        rd: u8,
    ) -> Result<(), TestCaseError> {
        prop_assume!(rs1 != rs2);
        prop_assume!(rs1 != rd);
        prop_assume!(rs2 != rd);
        let (program, record) = execute_code(
            [
                Instruction {
                    op: Op::SRA,
                    args: Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRA,
                    args: Args {
                        rd,
                        rs1,
                        imm: q,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(rs1, p), (rs2, q)],
        );
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn prove_sll_cpu(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_sll::<CpuStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
        #[test]
        fn prove_srl_cpu(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_srl::<CpuStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
        #[test]
        fn prove_sra_cpu(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_sra::<CpuStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_sll_mozak(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_sll::<MozakStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
        #[test]
        fn prove_srl_mozak(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_srl::<MozakStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
        #[test]
        fn prove_sra_mozak(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_sra::<MozakStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
    }
}
