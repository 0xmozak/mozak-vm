use plonky2::field::goldilocks_field::GoldilocksField;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct RegisterSelector {
    /// Register used for first operand.
    pub rs1: GoldilocksField,
    /// Register used for second operand.
    pub rs2: GoldilocksField,
    /// Register used for destination operand
    pub rd: GoldilocksField,
    /// Set 1 at index for register used in first operand, rest are 0.
    pub rs1_reg_sel: [GoldilocksField; 32],
    /// Set 1 at index for register used in second operand, rest are 0.
    pub rs2_reg_sel: [GoldilocksField; 32],
    /// Set 1 at index for register used in destination operand, rest are 0.
    pub rd_reg_sel: [GoldilocksField; 32],
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProcessorTraceRow {
    /// A processor clock value.
    pub clk: u32,
    /// All registers.
    pub registers: [GoldilocksField; 32],
    /// Register selectors as implemented in `RegisterSelector`.
    pub register_selectors: RegisterSelector,
    /// Program counter.
    pub pc: GoldilocksField,
    /// Opcode of instruction executed at given clock.
    pub opcode: u8,
    /// 1 => operand 2 is immediate value.
    pub op2_imm: GoldilocksField,
    /// Value of immediate operand (if instruction has one).
    pub imm_value: GoldilocksField,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Trace {
    pub processor_trace: Vec<ProcessorTraceRow>,
}
