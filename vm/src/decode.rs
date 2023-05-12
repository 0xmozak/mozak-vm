use crate::instruction::{ITypeInst, Instruction, JTypeInst, RTypeInst};

#[derive(Debug)]
pub enum OpCode {
    LB,
    LH,
    LW,
    LBU,
    LHU,
    ADDI,
    SLLI,
    SLTI,
    SLTIU,
    XORI,
    SRLI,
    SRAI,
    ORI,
    ANDI,
    AUIPC,
    SB,
    SH,
    SW,
    ADD,
    SUB,
    SLL,
    SLT,
    SLTU,
    XOR,
    SRL,
    SRA,
    OR,
    AND,
    MUL,
    MULH,
    MULU,
    MULSU,
    DIV,
    DIVU,
    REM,
    REMU,
    LUI,
    BEQ,
    BNE,
    BLT,
    BGE,
    BLTU,
    BGEU,
    JALR,
    JAL,
    ECALL,
    EBREAK,
    UNKNOWN,
}

/// Decode RS2 register number from 32-bit instruction
pub fn decode_rs2(word: u32) -> u8 {
    ((word & 0x01f00000) >> 20) as u8
}

/// Decode RS1 register number from 32-bit instruction
pub fn decode_rs1(word: u32) -> u8 {
    ((word & 0x000f8000) >> 15) as u8
}

/// Decode RD register number from 32-bit instruction
pub fn decode_rd(word: u32) -> u8 {
    ((word & 0x00000f80) >> 7) as u8
}

/// Decode Opcode from 32-bit instruction
pub fn decode_op(word: u32) -> u8 {
    (word & 0x0000007f) as u8
}

/// Decode func3 from 32-bit instruction
pub fn decode_func3(word: u32) -> u8 {
    ((word & 0x00007000) >> 12) as u8
}

/// Decode func7 from 32-bit instruction
pub fn decode_func7(word: u32) -> u8 {
    ((word & 0xfe000000) >> 25) as u8
}

/// Decode func12 from 32-bit instruction
pub fn decode_func12(word: u32) -> u16 {
    ((word & 0xfff00000) >> 20) as u16
}

/// Decode signed imm12 value
pub fn decode_imm12(word: u32) -> i16 {
    let val = ((word & 0xfff00000) >> 20) as u16;
    if (val & 0x0800) != 0 {
        // negative number
        let val = val - 1;
        return -((!val & 0x0fff) as i16);
    } else {
        return val as i16;
    }
}

/// Decode signed imm20 value for JTypeInst
/// Please refer RISCV manual section "Immediate Encoding Variants" for this
/// decoding
pub fn decode_imm20(word: u32) -> i32 {
    let val1 = ((word & 0x7FE00000) >> 20) as u32;
    let val2 = ((word & 0x00100000) >> 9) as u32;
    let val3 = (word & 0x000FF000) as u32;
    if (word & 0x80000000) != 0 {
        return (0xFFF00000 | val1 | val2 | val3) as i32;
    } else {
        return (0x00000000 | val1 | val2 | val3) as i32;
    }
}

pub fn decode_shamt(word: u32) -> u8 {
    ((word & 0x01f00000) >> 20) as u8
}

// Encodings can be verified against https://www.csl.cornell.edu/courses/ece5745/handouts/ece5745-tinyrv-isa.txt
pub fn decode(word: u32) -> OpCode {
    let opcode = word & 0x0000007f;
    let rs2 = (word & 0x01f00000) >> 20;
    let funct3 = (word & 0x00007000) >> 12;
    let funct7 = (word & 0xfe000000) >> 25;

    match opcode {
        0b0000011 => match funct3 {
            0x0 => OpCode::LB,
            0x1 => OpCode::LH,
            0x2 => OpCode::LW,
            0x4 => OpCode::LBU,
            0x5 => OpCode::LHU,
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        0b0010011 => match funct3 {
            0x0 => OpCode::ADDI,
            0x1 => OpCode::SLLI,
            0x2 => OpCode::SLTI,
            0x3 => OpCode::SLTIU,
            0x4 => OpCode::XORI,
            0x5 => match funct7 {
                0x00 => OpCode::SRLI,
                0x20 => OpCode::SRAI,
                _ => {
                    println!(
                        "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                        opcode, rs2, funct3, funct7
                    );
                    OpCode::UNKNOWN
                }
            },
            0x6 => OpCode::ORI,
            0x7 => OpCode::ANDI,
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        0b0010111 => OpCode::AUIPC,
        0b0100011 => match funct3 {
            0x0 => OpCode::SB,
            0x1 => OpCode::SH,
            0x2 => OpCode::SW,
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        0b0110011 => match (funct3, funct7) {
            (0x0, 0x00) => OpCode::ADD,
            (0x0, 0x20) => OpCode::SUB,
            (0x1, 0x00) => OpCode::SLL,
            (0x2, 0x00) => OpCode::SLT,
            (0x3, 0x00) => OpCode::SLTU,
            (0x4, 0x00) => OpCode::XOR,
            (0x5, 0x00) => OpCode::SRL,
            (0x5, 0x20) => OpCode::SRA,
            (0x6, 0x00) => OpCode::OR,
            (0x7, 0x00) => OpCode::AND,
            (0x0, 0x01) => OpCode::MUL,
            (0x1, 0x01) => OpCode::MULH,
            (0x2, 0x01) => OpCode::MULSU,
            (0x3, 0x01) => OpCode::MULU,
            (0x4, 0x01) => OpCode::DIV,
            (0x5, 0x01) => OpCode::DIVU,
            (0x6, 0x01) => OpCode::REM,
            (0x7, 0x01) => OpCode::REMU,
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        0b0110111 => OpCode::LUI,
        0b1100011 => match funct3 {
            0x0 => OpCode::BEQ,
            0x1 => OpCode::BNE,
            0x4 => OpCode::BLT,
            0x5 => OpCode::BGE,
            0x6 => OpCode::BLTU,
            0x7 => OpCode::BGEU,
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        0b1100111 => match funct3 {
            0x0 => OpCode::JALR,
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        0b1101111 => OpCode::JAL,
        0b1110011 => match funct3 {
            0x0 => match (rs2, funct7) {
                (0x0, 0x0) => OpCode::ECALL,
                (0x1, 0x0) => OpCode::EBREAK,
                _ => {
                    println!(
                        "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                        opcode, rs2, funct3, funct7
                    );
                    OpCode::UNKNOWN
                }
            },
            _ => {
                println!(
                    "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                    opcode, rs2, funct3, funct7
                );
                OpCode::UNKNOWN
            }
        },
        _ => {
            println!(
                "opcode: {:?}, rs2: {:?}, funct3: {:?}, funct7 {:?}",
                opcode, rs2, funct3, funct7
            );
            OpCode::UNKNOWN
        }
    }
}

pub fn decode_instruction(word: u32) -> Instruction {
    let opcode = decode_op(word);
    let funct3 = decode_func3(word);
    let funct7 = decode_func7(word);

    match opcode {
        0b0110011 => match (funct3, funct7) {
            (0x0, 0x00) => {
                let rs1 = decode_rs1(word);
                let rs2 = decode_rs2(word);
                let rd = decode_rd(word);
                return Instruction::ADD(RTypeInst { rs1, rs2, rd });
            }
            (0x0, 0x20) => {
                let rs1 = decode_rs1(word);
                let rs2 = decode_rs2(word);
                let rd = decode_rd(word);
                return Instruction::SUB(RTypeInst { rs1, rs2, rd });
            }
            (0x4, 0x00) => {
                let rs1 = decode_rs1(word);
                let rs2 = decode_rs2(word);
                let rd = decode_rd(word);
                return Instruction::XOR(RTypeInst { rs1, rs2, rd });
            }
            _ => return Instruction::UNKNOWN,
        },
        0b0000011 => match funct3 {
            0x0 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                return Instruction::LB(ITypeInst { rs1, rd, imm12 });
            }
            0x1 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                return Instruction::LH(ITypeInst { rs1, rd, imm12 });
            }
            0x2 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                return Instruction::LW(ITypeInst { rs1, rd, imm12 });
            }
            0x4 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                return Instruction::LBU(ITypeInst { rs1, rd, imm12 });
            }
            0x5 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                return Instruction::LHU(ITypeInst { rs1, rd, imm12 });
            }
            _ => return Instruction::UNKNOWN,
        },
        0b0010011 => match funct3 {
            0x0 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                return Instruction::ADDI(ITypeInst { rs1, rd, imm12 });
            }
            0x1 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let shamt = decode_shamt(word);
                return Instruction::SLLI(ITypeInst {
                    rs1,
                    rd,
                    imm12: shamt.into(),
                });
            }
            _ => return Instruction::UNKNOWN,
        },
        0b1110011 => match decode_func12(word) {
            0x0 => return Instruction::ECALL,
            0x1 => return Instruction::EBREAK,
            _ => return Instruction::UNKNOWN,
        },
        0b1101111 => {
            let rd = decode_rd(word);
            let imm20 = decode_imm20(word);
            return Instruction::JAL(JTypeInst { rd, imm20 });
        }
        _ => return Instruction::UNKNOWN,
    }
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use super::decode_instruction;
    use crate::instruction::{ITypeInst, Instruction, JTypeInst, RTypeInst};

    #[test_case(0x018B80B3, 1, 23, 24; "add r1, r23, r24")]
    #[test_case(0x00000033, 0, 0, 0; "add r0, r0, r0")]
    #[test_case(0x01FF8FB3, 31, 31, 31; "add r31, r31, r31")]
    fn add(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::ADD(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff18193, 3, 3, 2047; "addi r3, r3, 2047")]
    #[test_case(0x80018193, 3, 3, -2048; "addi r3, r3, -2048")]
    #[test_case(0x44800f93, 31, 0, 1096; "addi r31, r0, 1096")]
    #[test_case(0xdca58e13, 28, 11, -566; "addi r28, r11, -566")]
    fn addi(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::ADDI(ITypeInst { rs1, rd, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f21213, 4, 4, 31; "slli r4, r4, 31")]
    #[test_case(0x00769693, 13, 13, 7; "slli r13, r13, 7")]
    fn slli(word: u32, rd: u8, rs1: u8, shamt: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLLI(ITypeInst {
            rs1,
            rd,
            imm12: shamt.into(),
        });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x409401b3, 3, 8, 9; "sub r3, r8, r9")]
    #[test_case(0x407383b3, 7, 7, 7; "sub r7, r7, r7")]
    #[test_case(0x41bc8733, 14, 25, 27; "sub r14, r25, r27")]
    fn sub(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SUB(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x840000ef,1, -1048512; "jal r1, -1048512")]
    #[test_case(0x7c1ffa6f,20, 1048512; "jal r20, 1048512")]
    fn jal(word: u32, rd: u8, imm20: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::JAL(JTypeInst { rd, imm20 });
        assert_eq!(ins, match_ins);
    }
}
