#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct Data {
    pub rs1: u8,
    pub rs2: u8,
    pub rd: u8,
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
    SLLI,
    SLT,
    SLTI,
    SLTU,
    SLTIU,
    SRAI,
    SRLI,
    LB,
    LH,
    LW,
    LBU,
    LHU,
    XOR,
    XORI,
    JAL,
    JALR,
    BEQ,
    BNE,
    BLT,
    BGE,
    BLTU,
    BGEU,
    AND,
    ANDI,
    OR,
    ORI,
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
