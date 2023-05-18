#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RTypeInst {
    pub rs1: u8,
    pub rs2: u8,
    pub rd: u8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ITypeInst {
    pub rs1: u8,
    pub rd: u8,
    /// 12 bit sign extended immediate value
    /// -2048 to 2047
    pub imm12: i16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct STypeInst {
    pub rs1: u8,
    pub rs2: u8,
    /// 12 bit sign extended immediate value
    /// -2048 to 2047
    pub imm12: i16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct JTypeInst {
    pub rd: u8,
    /// 20 bit sign extended immediate offset
    /// value in multiples of 2 bytes.
    /// -1 MB to 1 MB
    pub imm20: i32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BTypeInst {
    pub rs1: u8,
    pub rs2: u8,
    /// 12 bit sign extended immediate offset
    /// value in multiples of 2 bytes.
    /// -4096 to 4095
    pub imm12: i16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Instruction {
    ADD(RTypeInst),
    ADDI(ITypeInst),
    SUB(RTypeInst),
    SRL(RTypeInst),
    SRA(RTypeInst),
    SLL(RTypeInst),
    SLLI(ITypeInst),
    SLT(RTypeInst),
    SLTU(RTypeInst),
    LB(ITypeInst),
    LH(ITypeInst),
    LW(ITypeInst),
    LBU(ITypeInst),
    LHU(ITypeInst),
    XOR(RTypeInst),
    JAL(JTypeInst),
    JALR(ITypeInst),
    BEQ(BTypeInst),
    BNE(BTypeInst),
    BLT(BTypeInst),
    BGE(BTypeInst),
    BLTU(BTypeInst),
    BGEU(BTypeInst),
    AND(RTypeInst),
    OR(RTypeInst),
    SW(STypeInst),
    SH(STypeInst),
    SB(STypeInst),
    MUL(RTypeInst),
    MULH(RTypeInst),
    MULHU(RTypeInst),
    MULHSU(RTypeInst),
    ECALL,
    EBREAK,
    UNKNOWN,
}
