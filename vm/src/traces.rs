use plonky2::field::{goldilocks_field::GoldilocksField, types::Field};
use serde::Serialize;

use crate::{
    instruction::{BTypeInst, ITypeInst, Instruction, JTypeInst, RTypeInst, STypeInst, UTypeInst},
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
        Self {
            rs1: GoldilocksField::from_canonical_u8(inst.rs1),
            rs2: GoldilocksField::from_canonical_u8(inst.rs2),
            rs1_reg_sel: init_arr(&[(inst.rs1, GoldilocksField::from_canonical_u8(1))]),
            rs2_reg_sel: init_arr(&[(inst.rs2, GoldilocksField::from_canonical_u8(1))]),
            ..Self::default()
        }
    }
}

impl From<&BTypeInst> for RegisterSelector {
    fn from(inst: &BTypeInst) -> Self {
        Self {
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
        Self {
            rs1: GoldilocksField::from_canonical_u8(inst.rs1),
            rs2: GoldilocksField::from_canonical_u8(inst.rs2),
            rd: GoldilocksField::from_canonical_u8(inst.rd),
            rs1_reg_sel: init_arr(&[(inst.rs1, GoldilocksField::from_canonical_u8(1))]),
            rs2_reg_sel: init_arr(&[(inst.rs2, GoldilocksField::from_canonical_u8(1))]),
            rd_reg_sel: init_arr(&[(inst.rd, GoldilocksField::from_canonical_u8(1))]),
        }
    }
}

impl From<&JTypeInst> for RegisterSelector {
    fn from(inst: &JTypeInst) -> Self {
        Self {
            rd: GoldilocksField::from_canonical_u8(inst.rd),
            rd_reg_sel: init_arr(&[(inst.rd, GoldilocksField::from_canonical_u8(1))]),
            ..Self::default()
        }
    }
}

impl From<&UTypeInst> for RegisterSelector {
    fn from(inst: &UTypeInst) -> Self {
        Self {
            rd: GoldilocksField::from_canonical_u8(inst.rd),
            rd_reg_sel: init_arr(&[(inst.rd, GoldilocksField::from_canonical_u8(1))]),
            ..Self::default()
        }
    }
}

impl From<&Instruction> for RegisterSelector {
    #[allow(clippy::match_same_arms)]
    fn from(inst: &Instruction) -> Self {
        match inst {
            Instruction::ADD(inst) => Self::from(inst),
            Instruction::ADDI(inst) => Self::from(inst),
            Instruction::SUB(inst) => Self::from(inst),
            Instruction::SRL(inst) => Self::from(inst),
            Instruction::SRA(inst) => Self::from(inst),
            Instruction::SLL(inst) => Self::from(inst),
            Instruction::SLLI(inst) => Self::from(inst),
            Instruction::SLT(inst) => Self::from(inst),
            Instruction::SLTI(inst) => Self::from(inst),
            Instruction::SLTU(inst) => Self::from(inst),
            Instruction::SLTIU(inst) => Self::from(inst),
            Instruction::SRAI(inst) => Self::from(inst),
            Instruction::SRLI(inst) => Self::from(inst),
            Instruction::LB(inst) => Self::from(inst),
            Instruction::LH(inst) => Self::from(inst),
            Instruction::LW(inst) => Self::from(inst),
            Instruction::LBU(inst) => Self::from(inst),
            Instruction::LHU(inst) => Self::from(inst),
            Instruction::XOR(inst) => Self::from(inst),
            Instruction::XORI(inst) => Self::from(inst),
            Instruction::JAL(inst) => Self::from(inst),
            Instruction::JALR(inst) => Self::from(inst),
            Instruction::BEQ(inst) => Self::from(inst),
            Instruction::BNE(inst) => Self::from(inst),
            Instruction::BLT(inst) => Self::from(inst),
            Instruction::BGE(inst) => Self::from(inst),
            Instruction::BLTU(inst) => Self::from(inst),
            Instruction::BGEU(inst) => Self::from(inst),
            Instruction::AND(inst) => Self::from(inst),
            Instruction::ANDI(inst) => Self::from(inst),
            Instruction::OR(inst) => Self::from(inst),
            Instruction::ORI(inst) => Self::from(inst),
            Instruction::SW(inst) => Self::from(inst),
            Instruction::SH(inst) => Self::from(inst),
            Instruction::SB(inst) => Self::from(inst),
            Instruction::MUL(inst) => Self::from(inst),
            Instruction::MULH(inst) => Self::from(inst),
            Instruction::MULHU(inst) => Self::from(inst),
            Instruction::MULHSU(inst) => Self::from(inst),
            Instruction::LUI(inst) => Self::from(inst),
            Instruction::AUIPC(inst) => Self::from(inst),
            Instruction::DIV(inst) => Self::from(inst),
            Instruction::DIVU(inst) => Self::from(inst),
            Instruction::REM(inst) => Self::from(inst),
            Instruction::REMU(inst) => Self::from(inst),
            Instruction::FENCE(inst) => Self::from(inst),
            Instruction::CSRRW(inst) => Self::from(inst),
            Instruction::CSRRS(inst) => Self::from(inst),
            Instruction::CSRRWI(inst) => Self::from(inst),
            // MRET,
            // ECALL,
            // EBREAK,
            // UNKNOWN,
            _ => Self::default(),
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
pub struct TraceRow {
    pub processor_trace: Vec<ProcessorTraceRow>,
    pub memory_trace: Vec<MemoryTraceRow>,
}
