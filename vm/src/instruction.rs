#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Add {
    pub rs1: u8,
    pub rs2: u8,
    pub rd: u8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Instruction {
    ADD(Add),
}
