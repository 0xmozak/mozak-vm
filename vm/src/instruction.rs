#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct Data {
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    pub imm: u32,
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
    JAL,
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

/// Adding 0 to register 0 is the official way to encode a noop in Risc-V.
pub const NOOP_PAIR: (Op, Data) = (
    Op::ADD,
    Data {
        rd: 0,
        rs1: 0,
        rs2: 0,
        imm: 0,
    },
);
/// Adding 0 to register 0 is the official way to encode a noop in Risc-V.
pub const NOOP: Instruction = Instruction {
    op: Op::ADD,
    data: Data {
        rd: 0,
        rs1: 0,
        rs2: 0,
        imm: 0,
    },
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct Instruction {
    pub op: Op,
    pub data: Data,
}

impl Instruction {
    #[must_use]
    pub fn new(op: Op, rd: u8, rs1: u8, rs2: u8, imm: u32) -> Self {
        Instruction {
            op,
            data: Data { rd, rs1, rs2, imm },
        }
    }
}
