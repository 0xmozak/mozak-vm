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
    FENCE,
    CSRRW,
    CSRRS,
    CSRRWI,
    MRET,
    ECALL,
    EBREAK,
    #[default]
    UNKNOWN,
}

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
