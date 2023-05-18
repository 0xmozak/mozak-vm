use crate::instruction::{BTypeInst, ITypeInst, Instruction, JTypeInst, RTypeInst, STypeInst};

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
        -((!val & 0x0fff) as i16)
    } else {
        val as i16
    }
}

/// Decode signed imm20 value for [`JTypeInst`]
/// Please refer RISCV manual section "Immediate Encoding Variants" for this
/// decoding
pub fn decode_imm20(word: u32) -> i32 {
    let val1 = (word & 0x7FE00000) >> 20;
    let val2 = (word & 0x00100000) >> 9;
    let val3 = word & 0x000FF000;
    if (word & 0x80000000) != 0 {
        (0xFFF00000 | val1 | val2 | val3) as i32
    } else {
        (val1 | val2 | val3) as i32
    }
}

pub fn decode_imm12_b_imm(word: u32) -> i16 {
    let val1 = (word & 0x00000F00) >> 7;
    let val2 = (word & 0x7E000000) >> 20;
    let val3 = (word & 0x00000080) << 4;
    if (word & 0x80000000) != 0 {
        (0xF000 | val1 | val2 | val3) as i16
    } else {
        (val1 | val2 | val3) as i16
    }
}

pub fn decode_imm12_s_imm(word: u32) -> i16 {
    let val1 = (word & 0x00000F80) >> 7;
    let val2 = (word & 0x7E000000) >> 20;
    if (word & 0x80000000) != 0 {
        (0xF800 | val1 | val2) as i16
    } else {
        (val1 | val2) as i16
    }
}

pub fn decode_shamt(word: u32) -> u8 {
    ((word & 0x01f00000) >> 20) as u8
}

pub fn decode_instruction(word: u32) -> Instruction {
    let opcode = decode_op(word);
    let funct3 = decode_func3(word);
    let funct7 = decode_func7(word);

    match opcode {
        0b0110011 => {
            let rs1 = decode_rs1(word);
            let rs2 = decode_rs2(word);
            let rd = decode_rd(word);
            match (funct3, funct7) {
                (0x0, 0x00) => Instruction::ADD(RTypeInst { rs1, rs2, rd }),
                (0x0, 0x20) => Instruction::SUB(RTypeInst { rs1, rs2, rd }),
                (0x1, 0x00) => Instruction::SLL(RTypeInst { rs1, rs2, rd }),
                (0x2, 0x00) => Instruction::SLT(RTypeInst { rs1, rs2, rd }),
                (0x3, 0x00) => Instruction::SLTU(RTypeInst { rs1, rs2, rd }),
                (0x4, 0x00) => Instruction::XOR(RTypeInst { rs1, rs2, rd }),
                (0x5, 0x00) => Instruction::SRL(RTypeInst { rs1, rs2, rd }),
                (0x5, 0x20) => Instruction::SRA(RTypeInst { rs1, rs2, rd }),
                (0x6, 0x00) => Instruction::OR(RTypeInst { rs1, rs2, rd }),
                (0x7, 0x00) => Instruction::AND(RTypeInst { rs1, rs2, rd }),
                _ => Instruction::UNKNOWN,
            }
        }
        0b0000011 => {
            let rs1 = decode_rs1(word);
            let rd = decode_rd(word);
            let imm12 = decode_imm12(word);
            match funct3 {
                0x0 => Instruction::LB(ITypeInst { rs1, rd, imm12 }),
                0x1 => Instruction::LH(ITypeInst { rs1, rd, imm12 }),
                0x2 => Instruction::LW(ITypeInst { rs1, rd, imm12 }),
                0x4 => Instruction::LBU(ITypeInst { rs1, rd, imm12 }),
                0x5 => Instruction::LHU(ITypeInst { rs1, rd, imm12 }),
                _ => Instruction::UNKNOWN,
            }
        }
        0b0100011 => {
            let rs1 = decode_rs1(word);
            let rs2 = decode_rs2(word);
            let imm12 = decode_imm12_s_imm(word);
            match funct3 {
                0x0 => Instruction::SB(STypeInst { rs1, rs2, imm12 }),
                0x1 => Instruction::SH(STypeInst { rs1, rs2, imm12 }),
                0x2 => Instruction::SW(STypeInst { rs1, rs2, imm12 }),
                _ => Instruction::UNKNOWN,
            }
        }
        0b0010011 => match funct3 {
            0x0 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                Instruction::ADDI(ITypeInst { rs1, rd, imm12 })
            }
            0x1 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let shamt = decode_shamt(word);
                Instruction::SLLI(ITypeInst {
                    rs1,
                    rd,
                    imm12: shamt.into(),
                })
            }
            _ => Instruction::UNKNOWN,
        },
        0b1110011 => match decode_func12(word) {
            0x0 => Instruction::ECALL,
            0x1 => Instruction::EBREAK,
            _ => Instruction::UNKNOWN,
        },
        0b1101111 => {
            let rd = decode_rd(word);
            let imm20 = decode_imm20(word);
            Instruction::JAL(JTypeInst { rd, imm20 })
        }
        0b1100111 => match funct3 {
            0x0 => {
                let rs1 = decode_rs1(word);
                let rd = decode_rd(word);
                let imm12 = decode_imm12(word);
                Instruction::JALR(ITypeInst { rs1, rd, imm12 })
            }
            _ => Instruction::UNKNOWN,
        },
        0b1100011 => {
            let rs1 = decode_rs1(word);
            let rs2 = decode_rs2(word);
            let imm12 = decode_imm12_b_imm(word);

            match funct3 {
                0x0 => Instruction::BEQ(BTypeInst { rs1, rs2, imm12 }),
                0x1 => Instruction::BNE(BTypeInst { rs1, rs2, imm12 }),
                0x4 => Instruction::BLT(BTypeInst { rs1, rs2, imm12 }),
                0x5 => Instruction::BGE(BTypeInst { rs1, rs2, imm12 }),
                0x6 => Instruction::BLTU(BTypeInst { rs1, rs2, imm12 }),
                0x7 => Instruction::BGEU(BTypeInst { rs1, rs2, imm12 }),
                _ => Instruction::UNKNOWN,
            }
        }
        _ => Instruction::UNKNOWN,
    }
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use super::decode_instruction;
    use crate::instruction::{BTypeInst, ITypeInst, Instruction, JTypeInst, RTypeInst, STypeInst};

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

    #[test_case(0x012892b3, 5, 17, 18; "sll r5, r17, r18")]
    fn sll(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLL(RTypeInst { rs1, rd, rs2 });
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

    #[test_case(0x013952b3, 5, 18, 19; "srl r5, r18, r19")]
    fn srl(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SRL(RTypeInst { rs1, rd, rs2 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x413952b3, 5, 18, 19; "sra r5, r18, r19")]
    fn sra(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SRA(RTypeInst { rs1, rd, rs2 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x013922b3, 5, 18, 19; "slt r5, r18, r19")]
    fn slt(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLT(RTypeInst { rs1, rd, rs2 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x013932b3, 5, 18, 19; "sltu r5, r18, r19")]
    fn sltu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLTU(RTypeInst { rs1, rd, rs2 });
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

    #[test_case(0x7ff88567,10, 17, 2047; "jalr r10, r17, 2047")]
    #[test_case(0x80058ae7,21, 11, -2048; "jalr r21, r11, -2048")]
    fn jalr(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::JALR(ITypeInst { rd, rs1, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80941063,8, 9, -4096; "bne r8, r9, -4096")]
    #[test_case(0x7e941fe3,8, 9, 4094; "bne r8, r9, 4094")]
    fn bne(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BNE(BTypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80940063,8, 9, -4096; "beq r8, r9, -4096")]
    #[test_case(0x7e940fe3,8, 9, 4094; "beq r8, r9, 4094")]
    fn beq(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BEQ(BTypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80944063,8, 9, -4096; "blt r8, r9, -4096")]
    #[test_case(0x7e944fe3,8, 9, 4094; "blt r8, r9, 4094")]
    fn blt(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BLT(BTypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80946063,8, 9, -4096; "bltu r8, r9, -4096")]
    #[test_case(0x7e946fe3,8, 9, 4094; "bltu r8, r9, 4094")]
    fn bltu(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BLTU(BTypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80945063,8, 9, -4096; "bge r8, r9, -4096")]
    #[test_case(0x7e945fe3,8, 9, 4094; "bge r8, r9, 4094")]
    fn bge(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BGE(BTypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80947063,8, 9, -4096; "bgeu r8, r9, -4096")]
    #[test_case(0x7e947fe3,8, 9, 4094; "bgeu r8, r9, 4094")]
    fn bgeu(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BGEU(BTypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128f533, 10, 17, 18; "and r10, r17, r18")]
    fn and(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::AND(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128e533, 10, 17, 18; "or r10, r17, r18")]
    fn or(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::OR(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a00023, 0, 10, -2048; "sb r10, -2048(r0)")]
    #[test_case(0x7ea00fa3, 0, 10, 2047; "sb r10, 2047(r0)")]
    fn sb(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SB(STypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a01023, 0, 10, -2048; "sh r10, -2048(r0)")]
    #[test_case(0x7ea01fa3, 0, 10, 2047; "sh r10, 2047(r0)")]
    fn sh(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SH(STypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a02023, 0, 10, -2048; "sw r10, -2048(r0)")]
    #[test_case(0x7ea02fa3, 0, 10, 2047; "sw r10, 2047(r0)")]
    fn sw(word: u32, rs1: u8, rs2: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SW(STypeInst { rs1, rs2, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0af83, 31, 1, 2047; "lw r31, 2047(r1)")]
    #[test_case(0x8000af83, 31, 1, -2048; "lw r31, -2048(r1)")]
    fn lw(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LW(ITypeInst { rs1, rd, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff09f83, 31, 1, 2047; "lh r31, 2047(r1)")]
    #[test_case(0x80009f83, 31, 1, -2048; "lh r31, -2048(r1)")]
    fn lh(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LH(ITypeInst { rs1, rd, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0df83, 31, 1, 2047; "lhu r31, 2047(r1)")]
    #[test_case(0x8000df83, 31, 1, -2048; "lhu r31, -2048(r1)")]
    fn lhu(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LHU(ITypeInst { rs1, rd, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff08f83, 31, 1, 2047; "lb r31, 2047(r1)")]
    #[test_case(0x80008f83, 31, 1, -2048; "lb r31, -2048(r1)")]
    fn lb(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LB(ITypeInst { rs1, rd, imm12 });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0cf83, 31, 1, 2047; "lbu r31, 2047(r1)")]
    #[test_case(0x8000cf83, 31, 1, -2048; "lbu r31, -2048(r1)")]
    fn lbu(word: u32, rd: u8, rs1: u8, imm12: i16) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LBU(ITypeInst { rs1, rd, imm12 });
        assert_eq!(ins, match_ins);
    }
}
