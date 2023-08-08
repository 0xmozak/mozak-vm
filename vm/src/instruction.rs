use array_concat::concat_arrays;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct Args {
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    pub imm: u32,
    pub branch_target: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
#[repr(u8)]
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

pub const SIGNED2_OPCODES: [Op; 8] = [
    Op::SLT,
    Op::LB,
    Op::LH,
    Op::BLT,
    Op::BGE,
    Op::DIV,
    Op::REM,
    Op::MULH,
];
pub const SIGNED1_OPCODES: [Op; SIGNED2_OPCODES.len() + 1] =
    concat_arrays!(SIGNED2_OPCODES, [Op::MULHSU]);

impl Op {
    #[must_use]
    pub fn is_signed1(&self) -> bool { SIGNED1_OPCODES.contains(self) }

    #[must_use]
    pub fn is_signed2(&self) -> bool { SIGNED2_OPCODES.contains(self) }
}

/// Adding 0 to register 0 is the official way to encode a noop in Risc-V.
pub const NOOP: Instruction = Instruction {
    op: Op::ADD,
    args: Args {
        rd: 0,
        rs1: 0,
        rs2: 0,
        imm: 0,
        branch_target: 0,
    },
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct Instruction {
    pub op: Op,
    pub args: Args,
}

impl Instruction {
    #[must_use]
    pub fn new(op: Op, args: Args) -> Self { Instruction { op, args } }
}
