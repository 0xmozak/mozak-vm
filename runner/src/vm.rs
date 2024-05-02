use anyhow::{anyhow, Result};
use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::elf::Program;
use crate::instruction::{Args, Instruction, Op};
use crate::state::{Aux, MemEntry, State};

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn mulh(a: u32, b: u32) -> u32 { ((i64::from(a as i32) * i64::from(b as i32)) >> 32) as u32 }

#[must_use]
pub fn mulhu(a: u32, b: u32) -> u32 { ((u64::from(a) * u64::from(b)) >> 32) as u32 }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn mulhsu(a: u32, b: u32) -> u32 { ((i64::from(a as i32) * i64::from(b)) >> 32) as u32 }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn div(a: u32, b: u32) -> u32 {
    match (a as i32, b as i32) {
        (_, 0) => 0xFFFF_FFFF,
        (a, b) => a.overflowing_div(b).0 as u32,
    }
}

#[must_use]
pub fn divu(a: u32, b: u32) -> u32 {
    match (a, b) {
        (_, 0) => 0xFFFF_FFFF,
        (a, b) => a / b,
    }
}

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn rem(a: u32, b: u32) -> u32 {
    (match (a as i32, b as i32) {
        (a, 0) => a,
        // overflow when -2^31 / -1
        (-0x8000_0000, -1) => 0,
        (a, b) => a % b,
    }) as u32
}

#[must_use]
pub fn remu(a: u32, b: u32) -> u32 {
    match (a, b) {
        (a, 0) => a,
        (a, b) => a % b,
    }
}

#[must_use]
pub fn dup(x: u32) -> (u32, u32) { (x, x) }

#[must_use]
pub fn lbu_raw(mem: &[u8; 4]) -> u32 { mem[0].into() }

#[must_use]
pub fn lbu(mem: &[u8; 4]) -> (u32, u32) { dup(lbu_raw(mem)) }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
pub fn lb(mem: &[u8; 4]) -> (u32, u32) {
    let raw = lbu_raw(mem);
    (raw, i32::from(raw as i8) as u32)
}

#[must_use]
pub fn lhu_raw(mem: &[u8; 4]) -> u32 { u16::from_le_bytes([mem[0], mem[1]]).into() }

#[must_use]
pub fn lhu(mem: &[u8; 4]) -> (u32, u32) { dup(lhu_raw(mem)) }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
pub fn lh(mem: &[u8; 4]) -> (u32, u32) {
    let raw = lhu_raw(mem);
    (raw, i32::from(raw as i16) as u32)
}

#[must_use]
pub fn lw(mem: &[u8; 4]) -> (u32, u32) { dup(u32::from_le_bytes(*mem)) }

impl<F: RichField> State<F> {
    #[must_use]
    pub fn jalr(self, inst: &Args) -> (Aux<F>, Self) {
        let new_pc = self.get_register_value(inst.rs1).wrapping_add(inst.imm) & !1;
        let dst_val = self.get_pc().wrapping_add(4);
        (
            Aux {
                dst_val,
                ..Default::default()
            },
            self.set_pc(new_pc).set_register_value(inst.rd, dst_val),
        )
    }

    #[must_use]
    /// # Panics
    ///
    /// Panics in case we intend to store to a read-only location
    /// TODO: Review the decision to panic.  We might also switch to using a
    /// Result, so that the caller can handle this.
    pub fn store(self, inst: &Args, bytes: u32) -> (Aux<F>, Self) {
        let mask = u32::MAX >> (32 - 8 * bytes);
        let raw_value: u32 = self.get_register_value(inst.rs1) & mask;
        let addr = self.get_register_value(inst.rs2).wrapping_add(inst.imm);
        let mem_addresses_used: Vec<u32> = (0..bytes).map(|i| addr.wrapping_add(i)).collect();
        (
            Aux {
                dst_val: raw_value,
                mem: Some(MemEntry { addr, raw_value }),
                mem_addresses_used,
                ..Default::default()
            },
            (0..bytes)
                .map(|i| addr.wrapping_add(i))
                .zip(raw_value.to_le_bytes())
                .fold(self, |acc, (i, byte)| acc.store_u8(i, byte).unwrap())
                .bump_pc(),
        )
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    /// # Errors
    ///
    /// Errors if the program contains an instruction with an unsupported
    /// opcode.
    pub fn execute_instruction(self, program: &Program) -> Result<(Aux<F>, Instruction, Self)> {
        let inst = self
            .current_instruction(program)
            .ok_or(anyhow!("Can't find instruction."))?
            .map_err(|e| {
                anyhow!(
                    "Unknown instruction {:x} at address {:x}",
                    e.instruction,
                    e.pc
                )
            })?;
        macro_rules! rop {
            ($op: expr) => {
                self.register_op(&inst.args, $op)
            };
        }
        // TODO: consider factoring out this logic from `register_op`, `branch_op`,
        // `memory_load` etc.
        let op1 = self.get_register_value(inst.args.rs1);
        let rs2_raw = self.get_register_value(inst.args.rs2);
        // For branch instructions, both op2 and imm serve different purposes.
        // Therefore, we avoid adding them together here.
        let op2 = if matches!(
            inst.op,
            Op::BEQ | Op::BNE | Op::BLT | Op::BLTU | Op::BGE | Op::BGEU
        ) {
            rs2_raw
        } else if matches!(inst.op, Op::SRL | Op::SLL | Op::SRA) {
            1u32 << (rs2_raw.wrapping_add(inst.args.imm) & 0b1_1111)
        } else {
            rs2_raw.wrapping_add(inst.args.imm)
        };

        let (aux, state) = match inst.op {
            Op::ADD => rop!(u32::wrapping_add),
            // Only use lower 5 bits of rs2 or imm
            Op::SLL => rop!(|a, b| a << (b & 0b1_1111)),
            // Only use lower 5 bits of rs2 or imm
            Op::SRL => rop!(|a, b| a >> (b & 0b1_1111)),
            // Only use lower 5 bits of rs2 or imm
            Op::SRA => rop!(|a, b| (a as i32 >> (b & 0b1_1111) as i32) as u32),
            Op::SLT => rop!(|a, b| u32::from((a as i32) < (b as i32))),
            Op::SLTU => rop!(|a, b| u32::from(a < b)),
            Op::AND => rop!(core::ops::BitAnd::bitand),
            Op::OR => rop!(core::ops::BitOr::bitor),
            Op::XOR => rop!(core::ops::BitXor::bitxor),
            Op::SUB => rop!(u32::wrapping_sub),

            Op::LB => self.memory_load(&inst.args, 1, lb),
            Op::LBU => self.memory_load(&inst.args, 1, lbu),
            Op::LH => self.memory_load(&inst.args, 2, lh),
            Op::LHU => self.memory_load(&inst.args, 2, lhu),
            Op::LW => self.memory_load(&inst.args, 4, lw),

            Op::ECALL => self.ecall(),
            Op::JALR => self.jalr(&inst.args),
            // branches
            Op::BEQ => self.branch_op(&inst.args, |a, b| a == b),
            Op::BNE => self.branch_op(&inst.args, |a, b| a != b),
            Op::BLT => self.branch_op(&inst.args, |a, b| (a as i32) < (b as i32)),
            Op::BLTU => self.branch_op(&inst.args, |a, b| a < b),
            Op::BGE => self.branch_op(&inst.args, |a, b| (a as i32) >= (b as i32)),
            Op::BGEU => self.branch_op(&inst.args, |a, b| a >= b),
            // branching done.
            Op::SW => self.store(&inst.args, 4),
            Op::SH => self.store(&inst.args, 2),
            Op::SB => self.store(&inst.args, 1),
            Op::MUL => rop!(u32::wrapping_mul),
            Op::MULH => rop!(mulh),
            Op::MULHU => rop!(mulhu),
            Op::MULHSU => rop!(mulhsu),
            Op::DIV => rop!(div),
            Op::DIVU => rop!(divu),
            Op::REM => rop!(rem),
            Op::REMU => rop!(remu),
        };
        Ok((
            Aux {
                new_pc: state.get_pc(),
                op1,
                op2,
                op2_raw: rs2_raw,
                ..aux
            },
            inst,
            state.bump_clock(),
        ))
    }
}

/// Each row corresponds to the state of the VM _just before_ executing the
/// instruction that the program counter points to.
#[derive(Debug, Clone)]
pub struct Row<F: RichField> {
    pub state: State<F>,
    pub aux: Aux<F>,
    pub instruction: Instruction,
}

impl<F: RichField> Row<F> {
    #[must_use]
    pub fn new(op: Op) -> Self {
        Row {
            state: State::default(),
            aux: Aux::default(),
            instruction: Instruction::new(op, Args::default()),
        }
    }
}

/// Unconstrained Trace produced by running the code
#[derive(Debug, Default)]
pub struct ExecutionRecord<F: RichField> {
    /// Each row holds the state of the vm and auxiliary
    /// information associated
    pub executed: Vec<Row<F>>,
    /// The last state of the vm before the program halts
    pub last_state: State<F>,
}

impl<F: RichField> ExecutionRecord<F> {
    /// Returns the state just before the final state
    #[must_use]
    pub fn state_before_final(&self) -> &State<F> { &self.executed[self.executed.len() - 2].state }
}

/// Execute a program
///
/// # Errors
/// This function returns an error, if an instruction could not be loaded
/// or executed.
///
/// # Panics
/// Panics in debug mode, when executing more steps than specified in
/// environment variable `MOZAK_MAX_LOOPS` at compile time.  Defaults to one
/// million steps.
/// This is a temporary measure to catch problems with accidental infinite
/// loops. (Matthias had some trouble debugging a problem with jumps
/// earlier.)
pub fn step<F: RichField>(
    program: &Program,
    mut last_state: State<F>,
) -> Result<ExecutionRecord<F>> {
    let mut executed = vec![];
    while !last_state.has_halted() {
        let (aux, instruction, new_state) = last_state.clone().execute_instruction(program)?;
        executed.push(Row {
            state: last_state,
            instruction,
            aux,
        });
        log::trace!("clk: {:?}, {:?}", new_state.clk, instruction);
        last_state = new_state;

        // 16777656
        if last_state.pc == 16_777_772 {
            log::warn!("Reached an unknown, {} {:?}", last_state.pc, last_state.current_instruction(program));
        }
        if cfg!(debug_assertions) {
            let limit: u64 = option_env!("MOZAK_MAX_LOOPS")
                .map_or(1_000_000, |env_var| env_var.parse().unwrap());
            if last_state.clk + 20 > limit {
                log::warn!(
                    "Almost looped for longer than MOZAK_MAX_LOOPS: {} {:?}",
                    last_state.pc,
                    last_state.current_instruction(program)
                );
            }
            debug_assert!(
                last_state.clk < limit,
                "Looped for longer than MOZAK_MAX_LOOPS",
            );
        }
    }
    if option_env!("MOZAK_COUNT_OPS").is_some() {
        println!("Instruction counts:");
        let total: u32 = executed.len().try_into().unwrap();
        println!("{:6.2?}%\t{total:10} total", 100_f64);
        for (count, op) in executed
            .iter()
            .map(|row| row.instruction.op)
            .sorted()
            .dedup_with_count()
            .sorted()
            .rev()
        {
            let count: u32 = count.try_into().unwrap();
            let percentage = 100_f64 * f64::from(count) / f64::from(total);
            println!("{percentage:6.2?}%\t{count:10} {op}");
        }
    }
    Ok(ExecutionRecord::<F> {
        executed,
        last_state,
    })
}

#[cfg(test)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use im::HashMap;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use proptest::prelude::ProptestConfig;
    use proptest::{prop_assume, proptest};

    use super::*;
    use crate::code;
    use crate::decode::ECALL;
    use crate::test_utils::{i16_extra, i32_extra, i8_extra, reg, u16_extra, u32_extra, u8_extra};

    fn simple_test_code(
        code: impl IntoIterator<Item = Instruction>,
        mem: &[(u32, u8)],
        regs: &[(u8, u32)],
    ) -> ExecutionRecord<GoldilocksField> {
        code::execute(code, mem, regs).1
    }

    fn divu_with_imm(rd: u8, rs1: u8, rs1_value: u32, imm: u32) {
        let e = simple_test_code(
            [Instruction::new(Op::DIVU, Args {
                rd,
                rs1,
                imm,
                ..Args::default()
            })],
            &[],
            &[(rs1, rs1_value)],
        );
        assert_eq!(
            e.state_before_final().get_register_value(rd),
            divu(rs1_value, imm)
        );
    }

    fn mul_with_imm(rd: u8, rs1: u8, rs1_value: u32, imm: u32) {
        let e = simple_test_code(
            [Instruction::new(Op::MUL, Args {
                rd,
                rs1,
                imm,
                ..Args::default()
            })],
            &[],
            &[(rs1, rs1_value)],
        );
        assert_eq!(
            e.state_before_final().get_register_value(rd),
            rs1_value.wrapping_mul(imm),
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { max_global_rejects: 100_000, .. Default::default() })]
        #[test]
        fn add_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let sum = rs1_value.wrapping_add(rs2_value);
            let e = simple_test_code(
                [Instruction::new(
                    Op::ADD,
                    Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), sum);
        }

        #[test]
        fn addi_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::ADD,
                    Args {
                        rd,
                        rs1,
                        imm,
                        ..Args::default()
                    }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(imm));
        }

        #[test]
        fn sll_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SLL,
                    Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                rs1_value << (rs2_value & 0b1_1111)
            );
        }

        #[test]
        fn and_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::AND,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                rs1_value & rs2_value
            );
        }

        #[test]
        fn andi_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::AND,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value & imm;
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                expected_value
            );
        }


        #[test]
        fn srl_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SRL,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                rs1_value >> (rs2_value & 0b1_1111)
            );
        }

        #[test]
        fn srli_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in 0..32u8) {
            // srli is implemented as DIVU with divisor being 1 << imm.
            divu_with_imm(rd, rs1, rs1_value, 1 << imm);
        }

        #[test]
        fn or_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::OR,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                rs1_value | rs2_value
            );
        }

        #[test]
        fn ori_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::OR,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value | imm;
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn xor_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::XOR,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                rs1_value ^ rs2_value
            );
        }

        #[test]
        fn xori_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::XOR,
                    Args { rd,
                    rs1,
                    imm ,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value ^ imm;
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn sra_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SRA,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                (rs1_value as i32 >> (rs2_value & 0b1_1111) as i32) as u32
            );
        }

        #[test]
        fn srai_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::SRA,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = (rs1_value as i32 >> (imm & 0b1_1111)) as u32;
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn slt_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SLT,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            let rs1_value = rs1_value as i32;
            let rs2_value = rs2_value as i32;
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                u32::from(rs1_value < rs2_value)
            );
        }

        #[test]
        fn sltu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SLTU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                e.state_before_final().get_register_value(rd),
                u32::from(rs1_value < rs2_value)
            );
        }

        #[test]
        fn slti_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::SLT,
                    Args { rd,
                    rs1,
                    imm,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), u32::from((rs1_value as i32) < (imm as i32)));
        }

        #[test]
        fn sltiu_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                [Instruction::new(
                    Op::SLTU,
                    Args { rd,
                    rs1,
                    imm,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), u32::from(rs1_value < imm));
        }

        #[test]
        fn slli_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in 0..32u8) {
            // slli is implemented as MUL with 1 << imm
            mul_with_imm(rd, rs1, rs1_value, 1 << imm);
        }

        #[test]
        fn lb_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                [Instruction::new(
                    Op::LB,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, memory_value as u8)],
                &[(rs2, rs2_value)]
            );

            let expected_value = i32::from(memory_value) as u32;
            assert_eq!(e.state_before_final().get_register_value(rd), expected_value);
        }

        #[test]
        fn lbu_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in u8_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                [Instruction::new(
                    Op::LBU,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, memory_value)],
                &[(rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), u32::from(memory_value));
        }

        #[test]
        fn lh_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in i16_extra()) {
            let address = rs2_value.wrapping_add(offset);
            let [mem0, mem1] = memory_value.to_le_bytes();

            let e = simple_test_code(
                [Instruction::new(
                    Op::LH,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, mem0), (address.wrapping_add(1), mem1)],
                &[(rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), i32::from(memory_value) as u32);
        }

        #[test]
        fn lhu_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in u16_extra()) {
            let address = rs2_value.wrapping_add(offset);
            let [mem0, mem1] = memory_value.to_le_bytes();

            let e = simple_test_code(
                [Instruction::new(
                    Op::LHU,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, mem0), (address.wrapping_add(1), mem1)],
                &[(rs2, rs2_value)]
            );

            assert_eq!(e.state_before_final().get_register_value(rd), u32::from(memory_value));
        }

        #[test]
        fn lw_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in u32_extra()) {
            let address = rs2_value.wrapping_add(offset);
            let [mem0, mem1, mem2, mem3] = memory_value.to_le_bytes();

            let e = simple_test_code(
                [Instruction::new(
                    Op::LW,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }
                )],
                &[(address, mem0), (address.wrapping_add(1), mem1), (address.wrapping_add(2), mem2), (address.wrapping_add(3), mem3)],
                &[(rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), memory_value);
        }

        #[test]
        fn sb_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs2_val.wrapping_add(offset);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SB,
                    Args {rs1,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );

            assert_eq!(u32::from(e.state_before_final().load_u8(address)), rs1_val & 0xff);
        }

        #[test]
        fn sh_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs2_val.wrapping_add(offset);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SH,
                    Args {
                    rs1,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );
            // lh will return [0, 1] as LSBs and will set MSBs to 0xFFFF
            let state = e.state_before_final();
            let (_, memory_value) = lh(
                &[
                    state.load_u8(address),
                    state.load_u8(address.wrapping_add(1)),
                    state.load_u8(address.wrapping_add(2)),
                    state.load_u8(address.wrapping_add(3))
                ]
            );
            assert_eq!(memory_value & 0xffff, rs1_val & 0xffff);
        }

        #[test]
        fn sw_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs2_val.wrapping_add(offset);
            let e = simple_test_code(
                [Instruction::new(
                    Op::SW,
                Args {
                    rs1,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );

            let state = e.state_before_final();
            let (_, memory_value) = lw(
                &[
                    state.load_u8(address),
                    state.load_u8(address.wrapping_add(1)),
                    state.load_u8(address.wrapping_add(2)),
                    state.load_u8(address.wrapping_add(3))
                ]
            );
            assert_eq!(memory_value, rs1_val);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mul_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod = rs1_value.wrapping_mul(rs2_value);
            let e = simple_test_code(
                [Instruction::new(
                    Op::MUL,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), prod);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mul_with_imm_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            mul_with_imm(rd, rs1, rs1_value, imm);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulh_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: i64 = i64::from(rs1_value) * i64::from(rs2_value);
            let e = simple_test_code(
                [Instruction::new(
                    Op::MULH,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulhu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: u64 = u64::from(rs1_value) * u64::from(rs2_value);
            let e = simple_test_code(
                [Instruction::new(
                    Op::MULHU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulhsu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: i64 = i64::from(rs1_value) * i64::from(rs2_value);
            let e = simple_test_code(
                [Instruction::new(
                    Op::MULHSU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn div_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let e = simple_test_code(
                [Instruction::new(
                    Op::DIV,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), div(rs1_value as u32, rs2_value as u32));
        }

        #[test]
        fn divu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let e = simple_test_code(
                [Instruction::new(
                    Op::DIVU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), divu(rs1_value, rs2_value));
        }

        #[test]
        fn divu_with_imm_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            prop_assume!(imm != 0);
            divu_with_imm(rd, rs1, rs1_value, imm);
        }

        #[test]
        fn rem_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            prop_assume!(rs1_value != i32::MIN && rs2_value != -1);
            let rem = rs1_value % rs2_value;
            let e = simple_test_code(
                [Instruction::new(
                    Op::REM,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rem as u32);
        }

        #[test]
        fn remu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let rem = rs1_value % rs2_value;
            let e = simple_test_code(
                [Instruction::new(
                    Op::REMU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rem);
        }

        #[test]
        fn beq_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            let e = simple_test_code(
                [  // rs1 == rs1: take imm-path (8)
                    Instruction::new(
                        Op::BEQ,
                        Args { rd,
                        rs1,
                        rs2: rs1,
                        imm: 8,  // branch target
                }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,

                    ..Args::default()
                }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn bne_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs1_value != rs2_value);

            let e = simple_test_code(
                [  // rs1 != rs2: take imm-path (8)
                    Instruction::new(
                        Op::BNE,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                    }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn blt_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value < rs2_value);

            let e = simple_test_code(
                [
                    Instruction::new(
                        Op::BLT,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                    }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args {rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                ],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(rs2_value) as u32);
        }

        #[test]
        fn bltu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value < rs2_value);

            let e = simple_test_code(
                [
                    Instruction::new(
                        Op::BLTU,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                    }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn bge_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value >= rs2_value);

            let e = simple_test_code(
                [
                    Instruction::new(
                        Op::BGE,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                        }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                ],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(rs2_value) as u32);
        }

        #[test]
        fn bgeu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value >= rs2_value);

            let e = simple_test_code(
                [
                    Instruction::new(
                        Op::BGEU,
                        Args {
                                    rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                        }
                    ),
                    Instruction::new(
                        Op::SUB,
                                Args {
                        rd: rs1,
                        rs1,
                        rs2,
                        imm: 0,  // branch target
                        }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(e.state_before_final().get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn jal_jalr_proptest(imm in 0_u32..3) {
            let imm_value_fixed = 4 * imm + 4; // 4 * (0..3) + 4 = 4, 8, 12, 16
            let inst = Instruction::new( // imm = 0, jump = 4, 1+1 + 1 + 1 + 1 = 5
                        Op::ADD,
                        Args {
                        rd: 2,
                        rs1: 2,
                        rs2: 3,
                    ..Args::default()
            }
                    );
            let e = simple_test_code(
                [
                    Instruction::new(
                        Op::JALR,
                        Args {
                            imm: imm_value_fixed,
                            ..Args::default()
                        }
                    ),
                    inst,
                    inst,
                    inst,
                    inst,
                ],
                &[],
                &[(2, 1), (3, 1)],
            );
            assert_eq!(e.state_before_final().get_register_value(2), 5 - imm);
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    fn simple_test(
        exit_at: u32,
        mem: &[(u32, u32)],
        regs: &[(u8, u32)],
    ) -> ExecutionRecord<GoldilocksField> {
        // TODO(Matthias): stick this line into proper common setup?
        let _ = env_logger::try_init();
        let exit_inst =
        // set sys-call EXIT in x17(or a7)
        &[(exit_at, 0x05d0_0893_u32),
        // add ECALL to halt the program
        (exit_at + 4, 0x0000_0073_u32)];

        let image: HashMap<u32, u32> = mem.iter().chain(exit_inst.iter()).copied().collect();
        let program = Program::from(image);

        let state = regs
            .iter()
            .fold(State::from(program.clone()), |state, (rs, val)| {
                state.set_register_value(*rs, *val)
            });

        let record = step(&program, state).unwrap();
        assert!(record.last_state.has_halted());
        record
    }

    // NOTE: For writing test cases please follow RISCV
    // calling convention for using registers in instructions.
    // Please check https://en.wikichip.org/wiki/risc-v/registers

    #[test]
    fn ecall() { let _ = simple_test_code([ECALL], &[], &[]); }

    #[test]
    fn lui() {
        // at 0 address instruction lui
        // LUI x1, -524288
        let ExecutionRecord { last_state, .. } = simple_test(4, &[(0_u32, 0x8000_00b7)], &[]);
        assert_eq!(last_state.get_register_value(1), 0x8000_0000);
        assert_eq!(last_state.get_register_value(1) as i32, -2_147_483_648);
    }

    #[test]
    fn auipc() {
        // at 0 address addi x0, x0, 0
        let ExecutionRecord { last_state, .. } = simple_test(
            8,
            &[
                (0_u32, 0x0000_0013),
                // at 4 address instruction auipc
                // auipc x1, -524288
                (4_u32, 0x8000_0097),
            ],
            &[],
        );
        assert_eq!(last_state.get_register_value(1), 0x8000_0004);
        assert_eq!(last_state.get_register_value(1) as i32, -2_147_483_644);
    }

    #[test]
    fn system_opcode_instructions() {
        let _ = simple_test(
            20,
            &[
                // mret
                (0_u32, 0x3020_0073),
                // csrrs, t5, mcause
                (4_u32, 0x3420_2f73),
                // csrrw, mtvec, t0
                (8_u32, 0x3052_9073),
                // csrrwi, 0x744, 8
                (12_u32, 0x7444_5073),
                // fence, iorw, iorw
                (16_u32, 0x0ff0_000f),
            ],
            &[],
        );
    }
}
