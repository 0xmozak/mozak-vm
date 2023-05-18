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
pub struct UTypeInst {
    pub rd: u8,
    /// 20 bit signed immediate offset
    /// -524288 to 524287 which will be
    /// placed in MSB, so shift by 12 bit
    /// to create 32 bit.
    /// so actual range is
    /// -2147483648 to 2147479552
    pub imm20: i32,
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
    SRAI(ITypeInst),
    SRLI(ITypeInst),
    LB(ITypeInst),
    LH(ITypeInst),
    LW(ITypeInst),
    LBU(ITypeInst),
    LHU(ITypeInst),
    XOR(RTypeInst),
    XORI(ITypeInst),
    JAL(JTypeInst),
    JALR(ITypeInst),
    BEQ(BTypeInst),
    BNE(BTypeInst),
    BLT(BTypeInst),
    BGE(BTypeInst),
    BLTU(BTypeInst),
    BGEU(BTypeInst),
    AND(RTypeInst),
    ANDI(ITypeInst),
    OR(RTypeInst),
    ORI(ITypeInst),
    SW(STypeInst),
    SH(STypeInst),
    SB(STypeInst),
    LUI(UTypeInst),
    AUIPC(UTypeInst),
    ECALL,
    EBREAK,
    UNKNOWN,
}
