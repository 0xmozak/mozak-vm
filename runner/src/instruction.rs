//! RV32I Base Integer Instructions + RV32M Multiply Extension
use derive_more::Display;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// Arguments of a Risc-V instruction
///
/// rd: destination register
/// rs1 & rs2: source registers
/// imm: immediate
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Args {
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    pub imm: u32,
}

/// Operands of RV32I + RV32M
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Display)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Op {
    // RV32I Base Integer Instructions
    // ADD: rd = rs1 + rs2 / ADDI: rd = rs1 + imm
    ADD,
    // SUB: rd = rs1 - rs2
    SUB,
    // XOR: rd = rs1 ^ rs2 / XOR Immediate: rd = rs1 ˆ imm
    XOR,
    // OR: rd = rs1 | rs2 / OR Immediate: rd = rs1 | imm
    OR,
    // AND: rd = rs1 & rs2 / AND Immediate: rd = rs1 & imm
    AND,
    // Shift Left Logical: rd = rs1 << rs2 /
    // Shift Left Logical Immediate: rd = rs1 << imm[0:4]
    SLL,
    // Shift Right Logical: rd = rs1 >> rs2 /
    // Shift Right Logical Immediate: rd = rs1 >> imm[0:4]
    SRL,
    // Shift Right Arithmetic: rd = rs1 >> rs2 /
    // Shift Right Arithmetic Immediate: rd = rs1 >> imm[0:4]
    SRA,
    // Set Less Than: rd = (rs1 < rs2)?1:0 /
    // Set Less Than Immediate: rd = (rs1 < imm)?1:0
    SLT,
    // Set Less Than (U): rd = (rs1 < rs2)?1:0 /
    // Set Less Than Immediate (U): rd = (rs1 < imm)?1:0
    SLTU,
    // Load Byte: rd = M[rs1+imm][0:7]
    LB,
    // Load Half: rd = M[rs1+imm][0:15]
    LH,
    // Load Word: rd = M[rs1+imm][0:31]
    LW,
    // Load Byte (U): rd = M[rs1+imm][0:7]
    LBU,
    // Load Half (U): rd = M[rs1+imm][0:15]
    LHU,
    // Store Byte: M[rs1+imm][0:7] = rs2[0:7]
    SB,
    // Store Half: M[rs1+imm][0:15] = rs2[0:15]
    SH,
    // Store Word: M[rs1+imm][0:31] = rs2[0:31]
    SW,
    // Branch == : if(rs1 == rs2) PC += imm
    BEQ,
    // Branch != : if(rs1 != rs2) PC += imm
    BNE,
    // Branch < : if(rs1 < rs2) PC += imm
    BLT,
    // Branch >= : if(rs1 >= rs2) PC += imm
    BGE,
    // Branch < (U) : if(rs1 < rs2) PC += imm
    BLTU,
    // Branch >= (U) : if(rs1 >= rs2) PC += imm
    BGEU,
    // Jump And Link Reg: rd = PC+4; PC = rs1 + imm
    JALR,

    // Environment Call: Transfer Control to OS
    ECALL,

    // RV32M Multiply Extension
    // MUL: rd = (rs1 * rs2)[31:0]
    MUL,
    // MUL High: rd = (rs1 * rs2)[63:32]
    MULH,
    // MUL High (S) (U): rd = (rs1 * rs2)[63:32]
    MULHU,
    // MUL High (U): rd = (rs1 * rs2)[63:32]
    MULHSU,
    // DIV: rd = (rs1 / rs2)
    DIV,
    // DIV (U): rd = (rs1 / rs2)
    DIVU,
    // Remainder: rd = (rs1 % rs2)
    REM,
    // Remainder (U): rd = (rs1 % rs2)
    REMU,

    #[default]
    UNKNOWN,
}

/// NOP Instruction in Risc-V is encoded as ADDI x0, x0, 0.
pub const NOOP: Instruction = Instruction {
    op: Op::ADD,
    args: Args {
        rd: 0,
        rs1: 0,
        rs2: 0,
        imm: 0,
    },
};

/// A RV32 [Instruction] with [Op] and [Args]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Instruction {
    pub op: Op,
    pub args: Args,
}

impl Instruction {
    /// Constructs a new [Instruction] with [Op] and [Args]
    #[must_use]
    pub fn new(op: Op, args: Args) -> Self { Instruction { op, args } }
}
