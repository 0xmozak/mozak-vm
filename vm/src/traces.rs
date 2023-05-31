use plonky2::field::{goldilocks_field::GoldilocksField, types::Field};
use serde::Serialize;

use crate::{
    instruction::{ITypeInst, RTypeInst, STypeInst},
    util::init_arr,
};

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

impl From<&ITypeInst> for RegisterSelector {
    fn from(inst: &ITypeInst) -> Self {
        RegisterSelector {
            rs1: GoldilocksField::from_canonical_u8(inst.rs1),
            rd: GoldilocksField::from_canonical_u8(inst.rd),
            rs1_reg_sel: init_arr(&[(inst.rs1, GoldilocksField::from_canonical_u8(1))]),
            rd_reg_sel: init_arr(&[(inst.rd, GoldilocksField::from_canonical_u8(1))]),
            ..Self::default()
        }
    }
}

impl From<&STypeInst> for RegisterSelector {
    fn from(inst: &STypeInst) -> Self {
        RegisterSelector {
            rs1: GoldilocksField::from_canonical_u8(inst.rs1),
            rs2: GoldilocksField::from_canonical_u8(inst.rs2),
            rs1_reg_sel: init_arr(&[(inst.rs1, GoldilocksField::from_canonical_u8(1))]),
            rs2_reg_sel: init_arr(&[(inst.rs2, GoldilocksField::from_canonical_u8(1))]),
            ..Self::default()
        }
    }
}

impl From<&RTypeInst> for RegisterSelector {
    fn from(inst: &RTypeInst) -> Self {
        RegisterSelector {
            rs1: GoldilocksField::from_canonical_u8(inst.rs1),
            rs2: GoldilocksField::from_canonical_u8(inst.rs2),
            rd: GoldilocksField::from_canonical_u8(inst.rd),
            rs1_reg_sel: init_arr(&[(inst.rs1, GoldilocksField::from_canonical_u8(1))]),
            rs2_reg_sel: init_arr(&[(inst.rs2, GoldilocksField::from_canonical_u8(1))]),
            rd_reg_sel: init_arr(&[(inst.rd, GoldilocksField::from_canonical_u8(1))]),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProcessorTraceRow {
    /// A processor clock value.
    pub clk: u32,
    pub registers: [GoldilocksField; 32],
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
pub struct MemoryTraceRow {
    /// A processor clock value.
    pub clk: u32,
    /// Address of memory used in instruction.
    pub address: GoldilocksField,
    /// Value at address on memory.
    pub value: GoldilocksField,
    /// 1 => Store, 0 => Load
    pub is_write: GoldilocksField,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Trace {
    pub processor_trace: Vec<ProcessorTraceRow>,
    pub memory_trace: Vec<MemoryTraceRow>,
}
