use im::HashMap;
#[cfg(not(feature = "no-std"))]
use mozak_runner::elf;
use serde::{Deserialize, Serialize};

/// A RISC program (same as mozak_runner::elf::Program)
/// We reimplement it here to avoid a dependency on mozak_runner
/// As the mozak_runner crate is unable to be compiled into "no_std" code
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TransitionFunction {
    /// The entrypoint of the program
    pub entry_point: u32,

    /// Read-only section of memory
    /// 'ro_memory' takes precedence, if a memory location is in both.
    pub ro_memory: Data,

    /// Read-write section of memory
    /// 'ro_memory' takes precedence, if a memory location is in both.
    pub rw_memory: Data,

    /// Executable code of the ELF, read only
    pub ro_code: Code,
}

#[cfg(not(feature = "no-std"))]
impl Into<Program> for TransitionFunction {
    fn into(self) -> Program {
        Program {
            entry_point: self.entry_point,
            ro_memory: self.ro_memory.0,
            rw_memory: self.rw_memory.0,
            ro_code: self.ro_code.0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Data(pub HashMap<u32, u8>);

#[cfg(not(feature = "no-std"))]
impl Into<elf::Data> for Data {
    fn into(self) -> elf::Data { elf::Data(self.0) }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Code(pub HashMap<u32, Instruction>);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Instruction {
    pub op: Op,
    pub args: Args,
}

#[cfg(not(feature = "no-std"))]
impl Into<elf::Code> for Code {
    fn into(self) -> elf::Code { elf::Code(self.0) }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
#[repr(u8)]
#[allow(clippy::all)]
pub enum Op {
    ADD,
    SUB,
    SRL,
    SRA,
    SLL,
    SLT,
    SLTU,
    LB,
    LH,
    LW,
    LBU,
    LHU,
    XOR,
    JALR,
    BEQ,
    BNE,
    BLT,
    BGE,
    BLTU,
    BGEU,
    AND,
    OR,
    SW,
    SH,
    SB,
    MUL,
    MULH,
    MULHU,
    MULHSU,
    DIV,
    DIVU,
    REM,
    REMU,
    ECALL,
    #[default]
    UNKNOWN,
}

#[cfg(not(feature = "no-std"))]
impl Into<elf::Op> for Op {
    fn into(self) -> elf::Op {
        match self {
            Op::ADD => elf::Op::ADD,
            Op::SUB => elf::Op::SUB,
            Op::SRL => elf::Op::SRL,
            Op::SRA => elf::Op::SRA,
            Op::SLL => elf::Op::SLL,
            Op::SLT => elf::Op::SLT,
            Op::SLTU => elf::Op::SLTU,
            Op::LB => elf::Op::LB,
            Op::LH => elf::Op::LH,
            Op::LW => elf::Op::LW,
            Op::LBU => elf::Op::LBU,
            Op::LHU => elf::Op::LHU,
            Op::XOR => elf::Op::XOR,
            Op::JALR => elf::Op::JALR,
            Op::BEQ => elf::Op::BEQ,
            Op::BNE => elf::Op::BNE,
            Op::BLT => elf::Op::BLT,
            Op::BGE => elf::Op::BGE,
            Op::BLTU => elf::Op::BLTU,
            Op::BGEU => elf::Op::BGEU,
            Op::AND => elf::Op::AND,
            Op::OR => elf::Op::OR,
            Op::SW => elf::Op::SW,
            Op::SH => elf::Op::SH,
            Op::SB => elf::Op::SB,
            Op::MUL => elf::Op::MUL,
            Op::MULH => elf::Op::MULH,
            Op::MULHU => elf::Op::MULHU,
            Op::MULHSU => elf::Op::MULHSU,
            Op::DIV => elf::Op::DIV,
            Op::DIVU => elf::Op::DIVU,
            Op::REM => elf::Op::REM,
            Op::REMU => elf::Op::REMU,
            Op::ECALL => elf::Op::ECALL,
            Op::UNKNOWN => elf::Op::UNKNOWN,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Args {
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    pub imm: u32,
}

#[cfg(not(feature = "no-std"))]
impl Into<elf::Args> for Args {
    fn into(self) -> elf::Args {
        elf::Args {
            rd: self.rd,
            rs1: self.rs1,
            rs2: self.rs2,
            imm: self.imm,
        }
    }
}
