use bitfield::bitfield;
use bitfield::BitRange;

use crate::instruction::{Instruction, Op, TypeInst};

/// Builds a i32 from segments, and right pads with zeroes
///
/// This function takes segment specifications in the same format as the table
/// in figure 2.4 of page 12 of <https://riscv.org/wp-content/uploads/2017/05/riscv-spec-v2.2.pdf>
///
/// So for example, a B-immediate takes:
///   segments: &[(31, 31), (7, 7), (30, 25), (11, 8)]
///   pad: 1
#[must_use]
fn extract_immediate(word: u32, segments: &[(usize, usize)], pad: usize) -> u32 {
    let len: usize = segments.iter().map(|(msb, lsb)| msb - lsb + 1).sum();
    let u = segments.iter().fold(0, |acc, (msb, lsb)| -> u32 {
        let bits: u32 = word.bit_range(*msb, *lsb);
        (acc << (msb - lsb + 1)) | bits
    });
    let bit_size = std::mem::size_of::<u32>() * 8;
    // shift back and forth for sign extension.
    (((u << (bit_size - len)) as i32) >> (bit_size - len - pad)) as u32
}

bitfield! {
    pub struct InstructionBits(u32);
    impl Debug;
    u8;
    pub opcode, _: 6, 0;
    pub rd, _: 11, 7;
    pub func3, _: 14, 12;
    pub rs1, _: 19, 15;
    pub rs2, _: 24, 20;
    pub shamt, _: 24, 20;
    pub func7, _: 31, 25;
    u16;
    pub func12, _: 31, 20;
}

#[must_use]
pub fn decode_instruction(word: u32) -> Instruction {
    let bf = InstructionBits(word);
    let rs1 = bf.rs1();
    let rs2 = bf.rs2();
    let rd = bf.rd();

    let stype = TypeInst {
        rs1,
        rs2,
        imm: extract_immediate(word, &[(31, 31), (30, 25), (11, 8), (7, 7)], 0),
        ..Default::default()
    };
    let rtype = TypeInst {
        rs1,
        rs2,
        rd,
        ..Default::default()
    };
    let itype = TypeInst {
        rs1,
        rd,
        imm: extract_immediate(word, &[(31, 20)], 0),
        ..Default::default()
    };
    let jtype = TypeInst {
        rd,
        imm: extract_immediate(word, &[(31, 31), (19, 12), (20, 20), (30, 25), (24, 21)], 1),
        ..Default::default()
    };
    let btype = TypeInst {
        rs1,
        rs2,
        imm: extract_immediate(word, &[(31, 31), (7, 7), (30, 25), (11, 8)], 1),
        ..Default::default()
    };
    let utype = TypeInst {
        rd,
        imm: extract_immediate(word, &[(31, 12)], 12),
        ..Default::default()
    };
    match bf.opcode() {
        0b011_0011 => match (bf.func3(), bf.func7()) {
            (0x0, 0x00) => Instruction {
                op: Op::ADD,
                data: rtype,
            },
            (0x0, 0x20) => Instruction {
                op: Op::SUB,
                data: rtype,
            },
            (0x1, 0x00) => Instruction {
                op: Op::SLL,
                data: rtype,
            },
            (0x2, 0x00) => Instruction {
                op: Op::SLT,
                data: rtype,
            },
            (0x3, 0x00) => Instruction {
                op: Op::SLTU,
                data: rtype,
            },
            (0x4, 0x00) => Instruction {
                op: Op::XOR,
                data: rtype,
            },
            (0x5, 0x00) => Instruction {
                op: Op::SRL,
                data: rtype,
            },
            (0x5, 0x20) => Instruction {
                op: Op::SRA,
                data: rtype,
            },
            (0x6, 0x00) => Instruction {
                op: Op::OR,
                data: rtype,
            },
            (0x7, 0x00) => Instruction {
                op: Op::AND,
                data: rtype,
            },
            (0x4, 0x01) => Instruction {
                op: Op::DIV,
                data: rtype,
            },
            (0x5, 0x01) => Instruction {
                op: Op::DIVU,
                data: rtype,
            },
            (0x6, 0x01) => Instruction {
                op: Op::REM,
                data: rtype,
            },
            (0x7, 0x01) => Instruction {
                op: Op::REMU,
                data: rtype,
            },
            (0x0, 0x01) => Instruction {
                op: Op::MUL,
                data: rtype,
            },
            (0x1, 0x01) => Instruction {
                op: Op::MULH,
                data: rtype,
            },
            (0x2, 0x01) => Instruction {
                op: Op::MULHSU,
                data: rtype,
            },
            (0x3, 0x01) => Instruction {
                op: Op::MULHU,
                data: rtype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b000_0011 => match bf.func3() {
            0x0 => Instruction {
                op: Op::LB,
                data: itype,
            },
            0x1 => Instruction {
                op: Op::LH,
                data: itype,
            },
            0x2 => Instruction {
                op: Op::LW,
                data: itype,
            },
            0x4 => Instruction {
                op: Op::LBU,
                data: itype,
            },
            0x5 => Instruction {
                op: Op::LHU,
                data: itype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b010_0011 => match bf.func3() {
            0x0 => Instruction {
                op: Op::SB,
                data: stype,
            },
            0x1 => Instruction {
                op: Op::SH,
                data: stype,
            },
            0x2 => Instruction {
                op: Op::SW,
                data: stype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b001_0011 => match bf.func3() {
            0x0 => Instruction {
                op: Op::ADDI,
                data: itype,
            },
            0x1 => Instruction {
                op: Op::SLLI,
                data: itype,
            },
            0x2 => Instruction {
                op: Op::SLTI,
                data: itype,
            },
            0x3 => Instruction {
                op: Op::SLTIU,
                data: itype,
            },
            0x4 => Instruction {
                op: Op::XORI,
                data: itype,
            },
            0x5 => {
                let imm = itype.imm;
                let imm_masked: u32 = imm.bit_range(4, 0);
                let itype = TypeInst {
                    imm: imm_masked,
                    ..itype
                };
                // Masks the first 7 bits in a word to differentiate between an
                // SRAI/SRLI instruction. They have the same funct3 value and are
                // differentiated by their 30th bit, for which SRAI = 1 and SRLI = 0.
                match imm.bit_range(11, 5) {
                    0b010_0000 => Instruction {
                        op: Op::SRAI,
                        data: itype,
                    },
                    0 => Instruction {
                        op: Op::SRLI,
                        data: itype,
                    },
                    _ => Instruction {
                        op: Op::UNKNOWN,
                        data: Default::default(),
                    },
                }
            }
            0x6 => Instruction {
                op: Op::ORI,
                data: itype,
            },
            0x7 => Instruction {
                op: Op::ANDI,
                data: itype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b111_0011 => match (bf.func3(), bf.func12()) {
            (0x0, 0x0) => Instruction {
                op: Op::ECALL,
                data: Default::default(),
            },
            (0x0, 0x302) => Instruction {
                op: Op::MRET,
                data: Default::default(),
            },
            (0x0, 0x1) => Instruction {
                op: Op::EBREAK,
                data: Default::default(),
            },
            (0x1, _) => Instruction {
                op: Op::CSRRW,
                data: itype,
            },
            (0x2, _) => Instruction {
                op: Op::CSRRS,
                data: itype,
            },
            (0x5, _) => Instruction {
                op: Op::CSRRWI,
                data: itype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b110_1111 => Instruction {
            op: Op::JAL,
            data: jtype,
        },
        0b110_0111 => match bf.func3() {
            0x0 => Instruction {
                op: Op::JALR,
                data: itype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b110_0011 => match bf.func3() {
            0x0 => Instruction {
                op: Op::BEQ,
                data: btype,
            },
            0x1 => Instruction {
                op: Op::BNE,
                data: btype,
            },
            0x4 => Instruction {
                op: Op::BLT,
                data: btype,
            },
            0x5 => Instruction {
                op: Op::BGE,
                data: btype,
            },
            0x6 => Instruction {
                op: Op::BLTU,
                data: btype,
            },
            0x7 => Instruction {
                op: Op::BGEU,
                data: btype,
            },
            _ => Instruction {
                op: Op::UNKNOWN,
                data: Default::default(),
            },
        },
        0b011_0111 => Instruction {
            op: Op::LUI,
            data: utype,
        },
        0b001_0111 => Instruction {
            op: Op::AUIPC,
            data: utype,
        },
        0b000_1111 => Instruction {
            op: Op::FENCE,
            data: itype,
        },
        _ => Instruction {
            op: Op::UNKNOWN,
            data: Default::default(),
        },
    }
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use super::{decode_instruction, extract_immediate};
    use crate::instruction::{Instruction, Op, TypeInst};

    #[test_case(0b000_1100, 3; "extract 3")]
    #[test_case(0b1101_1100, u32::MAX; "extract neg 1")]
    fn extract_simple(word: u32, x: u32) {
        let a: u32 = extract_immediate(word, &[(7, 6), (4, 2)], 0);
        assert_eq!(x, a);
    }
    #[test_case(0x018B_80B3, 1, 23, 24; "add r1, r23, r24")]
    #[test_case(0x0000_0033, 0, 0, 0; "add r0, r0, r0")]
    #[test_case(0x01FF_8FB3, 31, 31, 31; "add r31, r31, r31")]
    fn add(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::ADD,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff1_8193, 3, 3, 2047; "addi r3, r3, 2047")]
    #[test_case(0x8001_8193, 3, 3, -2048; "addi r3, r3, -2048")]
    #[test_case(0x4480_0f93, 31, 0, 1096; "addi r31, r0, 1096")]
    #[test_case(0xdca5_8e13, 28, 11, -566; "addi r28, r11, -566")]
    fn addi(word: u32, rd: u8, rs1: u8, imm: i32) {
        let imm = imm as u32;
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::ADDI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_92b3, 5, 17, 18; "sll r5, r17, r18")]
    fn sll(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SLL,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f2_1213, 4, 4, 31; "slli r4, r4, 31")]
    #[test_case(0x0076_9693, 13, 13, 7; "slli r13, r13, 7")]
    fn slli(word: u32, rd: u8, rs1: u8, shamt: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SLLI,
            data: TypeInst {
                rs1,
                rd,
                imm: shamt.into(),
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_52b3, 5, 18, 19; "srl r5, r18, r19")]
    fn srl(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SRL,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x4139_52b3, 5, 18, 19; "sra r5, r18, r19")]
    fn sra(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SRA,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_22b3, 5, 18, 19; "slt r5, r18, r19")]
    fn slt(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SLT,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x41f9_5293, 5, 18, 31; "srai r5, r18, 31")]
    fn srai(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SRAI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f9_5293, 5, 18, 31; "srli r5, r18, 31")]
    fn srli(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SRLI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff9_2293, 5, 18, 255; "slti r5, r18, 255")]
    fn slti(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SLTI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff9_3293, 5, 18, 255; "sltiu r5, r18, 255")]
    fn sltiu(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SLTIU,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_32b3, 5, 18, 19; "sltu r5, r18, r19")]
    fn sltu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SLTU,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x4094_01b3, 3, 8, 9; "sub r3, r8, r9")]
    #[test_case(0x4073_83b3, 7, 7, 7; "sub r7, r7, r7")]
    #[test_case(0x41bc_8733, 14, 25, 27; "sub r14, r25, r27")]
    fn sub(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::SUB,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8400_00ef,1, -1_048_512; "jal r1, -1048512")]
    #[test_case(0x7c1f_fa6f,20, 1_048_512; "jal r20, 1048512")]
    fn jal(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::JAL,
            data: TypeInst {
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff8_8567,10, 17, 2047; "jalr r10, r17, 2047")]
    #[test_case(0x8005_8ae7,21, 11, -2048; "jalr r21, r11, -2048")]
    fn jalr(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::JALR,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_1063,8, 9, -4096; "bne r8, r9, -4096")]
    #[test_case(0x7e94_1fe3,8, 9, 4094; "bne r8, r9, 4094")]
    fn bne(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::BNE,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_0063,8, 9, -4096; "beq r8, r9, -4096")]
    #[test_case(0x7e94_0fe3,8, 9, 4094; "beq r8, r9, 4094")]
    fn beq(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::BEQ,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_4063,8, 9, -4096; "blt r8, r9, -4096")]
    #[test_case(0x7e94_4fe3,8, 9, 4094; "blt r8, r9, 4094")]
    fn blt(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::BLT,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_6063,8, 9, -4096; "bltu r8, r9, -4096")]
    #[test_case(0x7e94_6fe3,8, 9, 4094; "bltu r8, r9, 4094")]
    fn bltu(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::BLTU,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_5063,8, 9, -4096; "bge r8, r9, -4096")]
    #[test_case(0x7e94_5fe3,8, 9, 4094; "bge r8, r9, 4094")]
    fn bge(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::BGE,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_7063,8, 9, -4096; "bgeu r8, r9, -4096")]
    #[test_case(0x7e94_7fe3,8, 9, 4094; "bgeu r8, r9, 4094")]
    fn bgeu(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::BGEU,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_f533, 10, 17, 18; "and r10, r17, r18")]
    fn and(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::AND,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff8_f513, 10, 17, 0xff; "andi r10, r17, 255")]
    fn andi(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::ANDI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8008_c513, 10, 17, -2048; "xori r10, r17, -2048")]
    fn xori(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::XORI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_e533, 10, 17, 18; "or r10, r17, r18")]
    fn or(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::OR,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff8_e513, 10, 17, 0xff; "ori r10, r17, 255")]
    fn ori(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::ORI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_0023, 0, 10, -2048; "sb r10, -2048(r0)")]
    #[test_case(0x7ea0_0fa3, 0, 10, 2047; "sb r10, 2047(r0)")]
    fn sb(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::SB,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_1023, 0, 10, -2048; "sh r10, -2048(r0)")]
    #[test_case(0x7ea0_1fa3, 0, 10, 2047; "sh r10, 2047(r0)")]
    fn sh(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::SH,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_2023, 0, 10, -2048; "sw r10, -2048(r0)")]
    #[test_case(0x7ea0_2fa3, 0, 10, 2047; "sw r10, 2047(r0)")]
    fn sw(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::SW,
            data: TypeInst {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_8533, 10, 17, 18; "mul r10, r17, r18")]
    fn mul(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::MUL,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_9533, 10, 17, 18; "mulh r10, r17, r18")]
    fn mulh(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::MULH,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_a533, 10, 17, 18; "mulhsu r10, r17, r18")]
    fn mulhsu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::MULHSU,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_b533, 10, 17, 18; "mulhu r10, r17, r18")]
    fn mulhu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::MULHU,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_af83, 31, 1, 2047; "lw r31, 2047(r1)")]
    #[test_case(0x8000_af83, 31, 1, -2048; "lw r31, -2048(r1)")]
    fn lw(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LW,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_9f83, 31, 1, 2047; "lh r31, 2047(r1)")]
    #[test_case(0x8000_9f83, 31, 1, -2048; "lh r31, -2048(r1)")]
    fn lh(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LH,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_df83, 31, 1, 2047; "lhu r31, 2047(r1)")]
    #[test_case(0x8000_df83, 31, 1, -2048; "lhu r31, -2048(r1)")]
    fn lhu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LHU,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_8f83, 31, 1, 2047; "lb r31, 2047(r1)")]
    #[test_case(0x8000_8f83, 31, 1, -2048; "lb r31, -2048(r1)")]
    fn lb(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LB,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_cf83, 31, 1, 2047; "lbu r31, 2047(r1)")]
    #[test_case(0x8000_cf83, 31, 1, -2048; "lbu r31, -2048(r1)")]
    fn lbu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LBU,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8000_00b7, 1, -2_147_483_648; "lui r1, -524288")]
    #[test_case(0x7fff_f0b7, 1, 2_147_479_552; "lui r1, 524287")]
    fn lui(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LUI,
            data: TypeInst {
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8000_0097, 1, -2_147_483_648; "auipc r1, -524288")]
    #[test_case(0x7fff_f097, 1, 2_147_479_552; "auipc r1, 524287")]
    fn auipc(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::AUIPC,
            data: TypeInst {
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_c533, 10, 17, 18; "div r10, r17, r18")]
    fn div(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::DIV,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_d533, 10, 17, 18; "divu r10, r17, r18")]
    fn divu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::DIVU,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_e533, 10, 17, 18; "rem r10, r17, r18")]
    fn rem(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::REM,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_f533, 10, 17, 18; "remu r10, r17, r18")]
    fn remu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::REMU,
            data: TypeInst {
                rs1,
                rs2,
                rd,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0000_0073; "ecall")]
    fn ecall(word: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::ECALL,
            data: Default::default(),
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff0_000f, 0, 0, 255; "fence, iorw, iorw")]
    fn fence(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::FENCE,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x3020_0073; "mret")]
    fn mret(word: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::MRET,
            data: Default::default(),
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x3420_2f73, 30, 0, 834; "csrrs, t5, mcause")]
    fn csrrs(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::CSRRS,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x3052_9073, 0, 5, 773; "csrrw, mtvec, t0")]
    fn csrrw(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::CSRRW,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7444_5073, 0, 8, 0x744; "csrrwi, 0x744, 8")]
    fn csrrwi(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction {
            op: Op::CSRRWI,
            data: TypeInst {
                rs1,
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }
}
