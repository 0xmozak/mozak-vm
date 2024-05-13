//! RV32I Base Integer Instructions + RV32M Multiply Extension
use serde::{Deserialize, Serialize};

/// Arguments of a RISC-V instruction
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Args {
    /// Destination Register
    pub rd: u8,
    /// Source Register
    pub rs1: u8,
    /// Source Register
    pub rs2: u8,
    /// Extracted Immediate
    pub imm: u32,
}

/// Operands of RV32I + RV32M
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[repr(u8)]
pub enum Op {
    // RV32I Base Integer Instructions
    /// ADD (Immediate): rd = rs1 + (rs2 + imm)
    ADD,
    /// SUB: rd = rs1 - rs2
    SUB,
    /// XOR (Immediate): rd = rs1 ^ (rs2 + imm)
    XOR,
    /// OR (Immediate): rd = rs1 | (rs2 + imm)
    OR,
    /// AND (Immediate): rd = rs1 & (rs2 + imm)
    AND,
    /// Shift Left Logical: rd = rs1 << rs2
    /// Shift Right Logical Immediate is handled as `MUL`
    SLL,
    /// Shift Right Logical (Immediate): rd = rs1 >> (rs2 + imm)
    SRL,
    /// Shift Right Arithmetic (Immediate): rd = rs1 >> (rs2 + imm)
    SRA,
    /// Set Less Than (Immediate): rd = (rs1 < (rs2 + imm))?1:0
    SLT,
    /// Set Less Than (Immediate) (U): rd = (rs1 < (rs2 + imm))?1:0
    SLTU,
    /// Load Byte: rd = M[rs1+imm]
    LB,
    /// Load Half: rd = M[rs1+imm]
    LH,
    /// Load Word: rd = M[rs1+imm]
    LW,
    /// Load Byte (U): rd = M[rs1+imm]
    LBU,
    /// Load Half (U): rd = M[rs1+imm]
    LHU,
    /// Store Byte: M[rs1+imm] = rs2
    SB,
    /// Store Half: M[rs1+imm] = rs2
    SH,
    /// Store Word: M[rs1+imm] = rs2
    SW,
    /// Branch == : if(rs1 == rs2) PC = imm
    BEQ,
    /// Branch != : if(rs1 != rs2) PC += imm
    BNE,
    /// Branch < : if(rs1 < rs2) PC += imm
    BLT,
    /// Branch >= : if(rs1 >= rs2) PC += imm
    BGE,
    /// Branch < (U) : if(rs1 < rs2) PC += imm
    BLTU,
    /// Branch >= (U) : if(rs1 >= rs2) PC += imm
    BGEU,
    /// Jump: rd = PC+4; PC += imm
    /// Jump And Link Reg: rd = PC+4; PC = rs1 + imm
    JALR,

    /// Environment Call: Transfer Control to OS
    ECALL,

    // RV32M Multiply Extension
    /// MUL: Place the lower 32 bits result of rs1 * rs2 in rd
    MUL,
    /// MUL High:
    /// Place the upper 32 bits result of signed rs1 * signed rs2 in rd
    MULH,
    /// MUL High (S) (U):
    /// Place the upper 32 bits result of unsigned rs1 * unsigned rs2 in rd
    MULHU,
    /// MUL High (U):
    /// Place the upper 32 bits result of signed rs1 * unsigned rs2 in rd
    MULHSU,
    /// DIV: rd = signed rs1 / signed rs2
    DIV,
    /// DIV (U): rd = unsigned rs1 / unsigned rs2
    DIVU,
    /// Remainder: rd = signed rs1 % signed rs2
    REM,
    /// Remainder (U): rd = unsigned rs1 % unsigned rs2
    REMU,
}

/// NOP Instruction in RISC-V is encoded as ADDI x0, x0, 0.
pub const NOP: Instruction = Instruction {
    op: Op::ADD,
    args: Args {
        rd: 0,
        rs1: 0,
        rs2: 0,
        imm: 0,
    },
};

/// Internal representation of a decoded RV32 [Instruction]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    /// Operand of Instruction
    pub op: Op,
    /// Arguments of Instruction
    pub args: Args,
}

impl Instruction {
    /// Creates a new [Instruction] given [Op] and [Args]
    #[must_use]
    pub fn new(op: Op, args: Args) -> Self { Instruction { op, args } }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecodingError {
    pub pc: u32,
    pub instruction: u32,
}
