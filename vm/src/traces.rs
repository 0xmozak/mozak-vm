use plonky2::field::goldilocks_field::GoldilocksField;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct RegisterSelector {
    pub rs1: GoldilocksField,
    pub rs2: GoldilocksField,
    pub rd: GoldilocksField,
    pub rs1_reg_sel: [GoldilocksField; 32],
    pub rs2_reg_sel: [GoldilocksField; 32],
    pub rd_reg_sel: [GoldilocksField; 32],
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProcessorTraceRow {
    pub clk: u32,
    pub registers: [GoldilocksField; 32],
    pub register_selectors: RegisterSelector,
    pub pc: GoldilocksField,
    pub opcode: u8,
    pub op1_imm: GoldilocksField,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Trace {
    pub processor_trace: Vec<ProcessorTraceRow>,
}
