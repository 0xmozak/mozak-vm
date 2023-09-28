extern crate alloc;

use alloc::vec::Vec;

#[cfg(feature = "std")]
use im::HashMap;
#[cfg(feature = "std")]
use mozak_runner::{elf, instruction};
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use sha3::Digest;

use crate::Id;

#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Transition {
    id: Id,
    pub program: Program,
}

impl Transition {
    #[cfg(feature = "std")]
    pub fn new(program: Program) -> Self {
        let id = Self::generate_id(program.clone());
        Self { id, program }
    }

    /// Generates a unique ID for the transition function.
    /// Currently, we use SHA3-256 hash function to generate the ID.
    #[cfg(feature = "std")]
    pub fn generate_id(program: Program) -> Id {
        let mut hasher = sha3::Sha3_256::new();
        hasher.update(<Vec<u8>>::from(program));
        let hash = hasher.finalize();

        Id(hash.into())
    }

    pub fn id(&self) -> Id { self.id }
}

impl From<Program> for Vec<u8> {
    fn from(program: Program) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&program.entry_point.to_be_bytes());
        // TODO - add code that converts the transition into bytes.
        bytes
    }
}

/// A RISC program (same as mozak_runner::elf::Program)
/// We reimplement it here to avoid a dependency on mozak_runner
/// As the mozak_runner crate is unable to be compiled into "no_std" code
#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Program {
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

#[cfg(feature = "std")]
impl From<Program> for elf::Program {
    fn from(transition: Program) -> Self {
        elf::Program {
            entry_point: transition.entry_point,
            ro_memory: transition.ro_memory.into(),
            rw_memory: transition.rw_memory.into(),
            ro_code: transition.ro_code.into(),
        }
    }
}

#[cfg(feature = "std")]
impl From<elf::Program> for Program {
    fn from(elf: elf::Program) -> Self {
        Self {
            entry_point: elf.entry_point,
            ro_memory: elf.ro_memory.into(),
            rw_memory: elf.rw_memory.into(),
            ro_code: elf.ro_code.into(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Data(pub Vec<(u32, u8)>);

#[cfg(feature = "std")]
impl From<Data> for elf::Data {
    fn from(data: Data) -> Self { elf::Data(HashMap::from(data.0)) }
}

#[cfg(feature = "std")]
impl From<elf::Data> for Data {
    fn from(data: elf::Data) -> Self { Self(data.0.into_iter().collect()) }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Code(pub Vec<(u32, Instruction)>);

#[cfg(feature = "std")]
impl From<Code> for elf::Code {
    fn from(code: Code) -> Self {
        elf::Code(HashMap::from(
            code.0
                .iter()
                .map(|(pos, inst)| (*pos, instruction::Instruction::from(*inst)))
                .collect::<Vec<_>>(),
        ))
    }
}

#[cfg(feature = "std")]
impl From<elf::Code> for Code {
    fn from(code: elf::Code) -> Self {
        Self(
            code.0
                .into_iter()
                .map(|(pos, inst)| (pos, Instruction::from(inst)))
                .collect(),
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Instruction {
    pub op: Op,
    pub args: Args,
}

#[cfg(feature = "std")]
impl From<Instruction> for instruction::Instruction {
    fn from(inst: Instruction) -> Self {
        instruction::Instruction {
            op: inst.op.into(),
            args: inst.args.into(),
        }
    }
}

#[cfg(feature = "std")]
impl From<instruction::Instruction> for Instruction {
    fn from(inst: instruction::Instruction) -> Self {
        Self {
            op: inst.op.into(),
            args: inst.args.into(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
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

#[cfg(feature = "std")]
impl From<Op> for instruction::Op {
    fn from(op: Op) -> Self {
        match op {
            Op::ADD => instruction::Op::ADD,
            Op::SUB => instruction::Op::SUB,
            Op::SRL => instruction::Op::SRL,
            Op::SRA => instruction::Op::SRA,
            Op::SLL => instruction::Op::SLL,
            Op::SLT => instruction::Op::SLT,
            Op::SLTU => instruction::Op::SLTU,
            Op::LB => instruction::Op::LB,
            Op::LH => instruction::Op::LH,
            Op::LW => instruction::Op::LW,
            Op::LBU => instruction::Op::LBU,
            Op::LHU => instruction::Op::LHU,
            Op::XOR => instruction::Op::XOR,
            Op::JALR => instruction::Op::JALR,
            Op::BEQ => instruction::Op::BEQ,
            Op::BNE => instruction::Op::BNE,
            Op::BLT => instruction::Op::BLT,
            Op::BGE => instruction::Op::BGE,
            Op::BLTU => instruction::Op::BLTU,
            Op::BGEU => instruction::Op::BGEU,
            Op::AND => instruction::Op::AND,
            Op::OR => instruction::Op::OR,
            Op::SW => instruction::Op::SW,
            Op::SH => instruction::Op::SH,
            Op::SB => instruction::Op::SB,
            Op::MUL => instruction::Op::MUL,
            Op::MULH => instruction::Op::MULH,
            Op::MULHU => instruction::Op::MULHU,
            Op::MULHSU => instruction::Op::MULHSU,
            Op::DIV => instruction::Op::DIV,
            Op::DIVU => instruction::Op::DIVU,
            Op::REM => instruction::Op::REM,
            Op::REMU => instruction::Op::REMU,
            Op::ECALL => instruction::Op::ECALL,
            Op::UNKNOWN => instruction::Op::UNKNOWN,
        }
    }
}

#[cfg(feature = "std")]
impl From<instruction::Op> for Op {
    fn from(op: instruction::Op) -> Self {
        match op {
            instruction::Op::ADD => Op::ADD,
            instruction::Op::SUB => Op::SUB,
            instruction::Op::SRL => Op::SRL,
            instruction::Op::SRA => Op::SRA,
            instruction::Op::SLL => Op::SLL,
            instruction::Op::SLT => Op::SLT,
            instruction::Op::SLTU => Op::SLTU,
            instruction::Op::LB => Op::LB,
            instruction::Op::LH => Op::LH,
            instruction::Op::LW => Op::LW,
            instruction::Op::LBU => Op::LBU,
            instruction::Op::LHU => Op::LHU,
            instruction::Op::XOR => Op::XOR,
            instruction::Op::JALR => Op::JALR,
            instruction::Op::BEQ => Op::BEQ,
            instruction::Op::BNE => Op::BNE,
            instruction::Op::BLT => Op::BLT,
            instruction::Op::BGE => Op::BGE,
            instruction::Op::BLTU => Op::BLTU,
            instruction::Op::BGEU => Op::BGEU,
            instruction::Op::AND => Op::AND,
            instruction::Op::OR => Op::OR,
            instruction::Op::SW => Op::SW,
            instruction::Op::SH => Op::SH,
            instruction::Op::SB => Op::SB,
            instruction::Op::MUL => Op::MUL,
            instruction::Op::MULH => Op::MULH,
            instruction::Op::MULHU => Op::MULHU,
            instruction::Op::MULHSU => Op::MULHSU,
            instruction::Op::DIV => Op::DIV,
            instruction::Op::DIVU => Op::DIVU,
            instruction::Op::REM => Op::REM,
            instruction::Op::REMU => Op::REMU,
            instruction::Op::ECALL => Op::ECALL,
            instruction::Op::UNKNOWN => Op::UNKNOWN,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Args {
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    pub imm: u32,
}

#[cfg(feature = "std")]
impl From<Args> for instruction::Args {
    fn from(args: Args) -> Self {
        instruction::Args {
            rd: args.rd,
            rs1: args.rs1,
            rs2: args.rs2,
            imm: args.imm,
        }
    }
}

#[cfg(feature = "std")]
impl From<instruction::Args> for Args {
    fn from(args: instruction::Args) -> Self {
        Self {
            rd: args.rd,
            rs1: args.rs1,
            rs2: args.rs2,
            imm: args.imm,
        }
    }
}
