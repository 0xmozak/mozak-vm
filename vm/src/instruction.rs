#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Add {
    pub rs1: u8,
    pub rs2: u8,
    pub rd: u8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AddI {
    pub rs1: u8,
    pub rd: u8,
    // 12 bit sign extended immediate value
    // -2048 to 2047
    pub imm12: i16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Instruction {
    ADD(Add),
    ADDI(AddI),
}
