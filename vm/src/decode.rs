use bitfield::{bitfield, BitRange};

use crate::instruction::{Args, Instruction, Op, NOOP};

/// Builds a i32 from segments, and right pads with zeroes
///
/// This function takes segment specifications in the same format as the table
/// in figure 2.4 of page 12 of <https://riscv.org/wp-content/uploads/2017/05/riscv-spec-v2.2.pdf>
///
/// So for example, a B-immediate takes:
///   segments: &[(31, 31), (7, 7), (30, 25), (11, 8)]
///   pad: 1
#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
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

#[allow(clippy::too_many_lines)]
#[allow(clippy::module_name_repetitions)]
#[allow(clippy::similar_names)]
#[must_use]
// NOTE(Matthias): If we ever split this into an extra compilation step, then
// the base version of `decode_instruction` doesn't need the extra pc parameter.
pub fn decode_instruction(pc: u32, word: u32) -> Instruction {
    let bf = InstructionBits(word);
    let rs1 = bf.rs1();
    let rs2 = bf.rs2();
    let rd = bf.rd();

    let stype = Args {
        rs1,
        rs2,
        imm: extract_immediate(word, &[(31, 31), (30, 25), (11, 8), (7, 7)], 0),
        ..Default::default()
    };
    let rtype = Args {
        rd,
        rs1,
        rs2,
        ..Default::default()
    };
    let itype = Args {
        rs1,
        rd,
        imm: extract_immediate(word, &[(31, 20)], 0),
        ..Default::default()
    };
    // jump type
    let jtype = Args {
        rd,
        // NOTE(Matthias): we use absolute addressing here.
        imm: extract_immediate(word, &[(31, 31), (19, 12), (20, 20), (30, 25), (24, 21)], 1)
            .wrapping_add(pc),
        ..Default::default()
    };
    // branch type
    let btype = Args {
        rs1,
        rs2,
        // NOTE(Matthias): we use absolute addressing here.
        branch_target: extract_immediate(word, &[(31, 31), (7, 7), (30, 25), (11, 8)], 1)
            .wrapping_add(pc),
        ..Default::default()
    };
    let utype = Args {
        rd,
        imm: extract_immediate(word, &[(31, 12)], 12),
        ..Default::default()
    };
    let utype_absolute = Args {
        rd,
        imm: extract_immediate(word, &[(31, 12)], 12).wrapping_add(pc),
        ..Default::default()
    };
    let noop = (NOOP.op, NOOP.args);

    let (op, args) = match bf.opcode() {
        0b011_0011 => match (bf.func3(), bf.func7()) {
            (0x0, 0x00) => (Op::ADD, rtype),
            (0x0, 0x20) => (Op::SUB, rtype),
            (0x1, 0x00) => (Op::SLL, rtype),
            (0x2, 0x00) => (Op::SLT, rtype),
            (0x3, 0x00) => (Op::SLTU, rtype),
            (0x4, 0x00) => (Op::XOR, rtype),
            (0x5, 0x00) => (Op::SRL, rtype),
            (0x5, 0x20) => (Op::SRA, rtype),
            (0x6, 0x00) => (Op::OR, rtype),
            (0x7, 0x00) => (Op::AND, rtype),
            (0x4, 0x01) => (Op::DIV, rtype),
            (0x5, 0x01) => (Op::DIVU, rtype),
            (0x6, 0x01) => (Op::REM, rtype),
            (0x7, 0x01) => (Op::REMU, rtype),
            (0x0, 0x01) => (Op::MUL, rtype),
            (0x1, 0x01) => (Op::MULH, rtype),
            (0x2, 0x01) => (Op::MULHSU, rtype),
            (0x3, 0x01) => (Op::MULHU, rtype),
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        0b000_0011 => match bf.func3() {
            0x0 => (Op::LB, itype),
            0x1 => (Op::LH, itype),
            0x2 => (Op::LW, itype),
            0x4 => (Op::LBU, itype),
            0x5 => (Op::LHU, itype),
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        0b010_0011 => match bf.func3() {
            0x0 => (Op::SB, stype),
            0x1 => (Op::SH, stype),
            0x2 => (Op::SW, stype),
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        0b001_0011 => match bf.func3() {
            // For Risc-V its ADDI but we handle it as ADD.
            0x0 => (Op::ADD, itype),
            // For Risc-V its SLLI but we handle it as SLL.
            0x1 if 0 == itype.imm & !0b1_1111 => (Op::SLL, itype),
            // For Risc-V its SLTI but we handle it as SLT.
            0x2 => (Op::SLT, itype),
            // For Risc-V its SLTIU but we handle it as SLTU.
            0x3 => (Op::SLTU, itype),
            // For Risc-V its XORI but we handle it as XOR.
            0x4 => (Op::XOR, itype),
            0x5 => {
                let imm = itype.imm;
                let imm_masked: u32 = imm.bit_range(4, 0);
                let itype = Args {
                    imm: imm_masked,
                    ..itype
                };
                // Masks the first 7 bits in a word to differentiate between an
                // SRAI/SRLI instruction. They have the same funct3 value and are
                // differentiated by their 30th bit, for which SRAI = 1 and SRLI = 0.
                match imm.bit_range(11, 5) {
                    // For Risc-V its SRAI but we handle it as SRA.
                    0b010_0000 => (Op::SRA, itype),
                    // For Risc-V its SRLI but we handle it as SRL.
                    0 => (Op::SRL, itype),
                    #[tarpaulin::skip]
                    _ => Default::default(),
                }
            }
            // For Risc-V its ORI but we handle it as OR.
            0x6 => (Op::OR, itype),
            // For Risc-V its ANDI but we handle it as AND.
            0x7 => (Op::AND, itype),
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        #[allow(clippy::match_same_arms)]
        0b111_0011 => match (bf.func3(), bf.func12()) {
            (0x0, 0x0) => (Op::ECALL, Args::default()),
            // For RISC-V this would be MRET,
            // but so far we implemented it as a no-op.
            #[tarpaulin::skip]
            (0x0, 0x302) => noop,
            // For RISC-V this would be EBREAK,
            // but so far we implemented it as a no-op.
            #[tarpaulin::skip]
            (0x0, 0x1) => noop,
            // For RISC-V this would be (Op::CSRRW, itype),
            // but so far we implemented it as a no-op.
            (0x1, _) => noop,
            // For RISC-V this would be (Op::CSRRS, itype),
            // but so far we implemented it as a no-op.
            (0x2, _) => noop,
            // For RISC-V this would be (Op::CSRRWI, itype),
            // but so far we implemented it as a no-op.
            (0x5, _) => noop,
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        // For Risc-V its JAL, but we handle it as JALR.
        0b110_1111 => (Op::JALR, jtype),
        0b110_0111 => match bf.func3() {
            0x0 => (Op::JALR, itype),
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        0b110_0011 => match bf.func3() {
            0x0 => (Op::BEQ, btype),
            0x1 => (Op::BNE, btype),
            0x4 => (Op::BLT, btype),
            0x5 => (Op::BGE, btype),
            0x6 => (Op::BLTU, btype),
            0x7 => (Op::BGEU, btype),
            #[tarpaulin::skip]
            _ => Default::default(),
        },
        // LUI in RISC-V; but our ADD instruction is general enough to express the same semantics
        // without a new op-code.
        0b011_0111 => (Op::ADD, utype),
        // AUIPC in RISC-V; but our ADD instruction is general enough to express the same semantics
        // without a new op-code.
        0b001_0111 => (Op::ADD, utype_absolute),
        // For RISC-V this would be (Op::FENCE, itype)
        // but so far we implemented it as a no-op.
        0b000_1111 => noop,
        #[tarpaulin::skip]
        _ => Default::default(),
    };

    Instruction::new(op, args)
}

#[cfg(test)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use proptest::prelude::*;
    use test_case::test_case;

    use super::{decode_instruction, extract_immediate};
    use crate::instruction::{Args, Instruction, Op, NOOP};
    use crate::test_utils::u32_extra;

    proptest! {
        /// This just tests that we don't panic during decoding.
        #[test]
        fn fuzz_decode(pc in u32_extra(), word in u32_extra()) {
            let _ = decode_instruction(pc, word);
        }
    }

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
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::ADD,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff1_8193, 3, 3, 2047; "addi r3, r3, 2047")]
    #[test_case(0x8001_8193, 3, 3, - 2048; "addi r3, r3, -2048")]
    #[test_case(0x4480_0f93, 31, 0, 1096; "addi r31, r0, 1096")]
    #[test_case(0xdca5_8e13, 28, 11, - 566; "addi r28, r11, -566")]
    fn addi(word: u32, rd: u8, rs1: u8, imm: i32) {
        let imm = imm as u32;
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::ADD,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_92b3, 5, 17, 18; "sll r5, r17, r18")]
    fn sll(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SLL,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f2_1213, 4, 4, 31; "slli r4, r4, 31")]
    #[test_case(0x0076_9693, 13, 13, 7; "slli r13, r13, 7")]
    fn slli(word: u32, rd: u8, rs1: u8, shamt: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SLL,
            args: Args {
                rd,
                rs1,
                imm: shamt.into(),
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_52b3, 5, 18, 19; "srl r5, r18, r19")]
    fn srl(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SRL,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x4139_52b3, 5, 18, 19; "sra r5, r18, r19")]
    fn sra(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SRA,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_22b3, 5, 18, 19; "slt r5, r18, r19")]
    fn slt(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SLT,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x41f9_5293, 5, 18, 31; "srai r5, r18, 31")]
    fn srai(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SRA,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f9_5293, 5, 18, 31; "srli r5, r18, 31")]
    fn srli(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SRL,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff9_2293, 5, 18, 255; "slti r5, r18, 255")]
    fn slti(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SLT,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff9_3293, 5, 18, 255; "sltiu r5, r18, 255")]
    fn sltiu(word: u32, rd: u8, rs1: u8, imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SLTU,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_32b3, 5, 18, 19; "sltu r5, r18, r19")]
    fn sltu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SLTU,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x4094_01b3, 3, 8, 9; "sub r3, r8, r9")]
    #[test_case(0x4073_83b3, 7, 7, 7; "sub r7, r7, r7")]
    #[test_case(0x41bc_8733, 14, 25, 27; "sub r14, r25, r27")]
    fn sub(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::SUB,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8400_00ef, 1, - 1_048_512; "jal r1, -1048512")]
    #[test_case(0x7c1f_fa6f, 20, 1_048_512; "jal r20, 1048512")]
    fn jal(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::JALR,
            args: Args {
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff8_8567, 10, 17, 2047; "jalr r10, r17, 2047")]
    #[test_case(0x8005_8ae7, 21, 11, - 2048; "jalr r21, r11, -2048")]
    fn jalr(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::JALR,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_1063, 8, 9, - 4096; "bne r8, r9, -4096")]
    #[test_case(0x7e94_1fe3, 8, 9, 4094; "bne r8, r9, 4094")]
    fn bne(word: u32, rs1: u8, rs2: u8, branch_target: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let branch_target = branch_target as u32;
        let match_ins = Instruction {
            op: Op::BNE,
            args: Args {
                rs1,
                rs2,
                branch_target,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_0063, 8, 9, - 4096; "beq r8, r9, -4096")]
    #[test_case(0x7e94_0fe3, 8, 9, 4094; "beq r8, r9, 4094")]
    fn beq(word: u32, rs1: u8, rs2: u8, branch_target: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let branch_target = branch_target as u32;
        let match_ins = Instruction {
            op: Op::BEQ,
            args: Args {
                rs1,
                rs2,
                branch_target,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_4063, 8, 9, - 4096; "blt r8, r9, -4096")]
    #[test_case(0x7e94_4fe3, 8, 9, 4094; "blt r8, r9, 4094")]
    fn blt(word: u32, rs1: u8, rs2: u8, branch_target: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let branch_target: u32 = branch_target as u32;
        let match_ins = Instruction {
            op: Op::BLT,
            args: Args {
                rs1,
                rs2,
                branch_target,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_6063, 8, 9, - 4096; "bltu r8, r9, -4096")]
    #[test_case(0x7e94_6fe3, 8, 9, 4094; "bltu r8, r9, 4094")]
    fn bltu(word: u32, rs1: u8, rs2: u8, branch_target: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let branch_target = branch_target as u32;
        let match_ins = Instruction {
            op: Op::BLTU,
            args: Args {
                rs1,
                rs2,
                branch_target,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_5063, 8, 9, - 4096; "bge r8, r9, -4096")]
    #[test_case(0x7e94_5fe3, 8, 9, 4094; "bge r8, r9, 4094")]
    fn bge(word: u32, rs1: u8, rs2: u8, branch_target: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let branch_target = branch_target as u32;
        let match_ins = Instruction {
            op: Op::BGE,
            args: Args {
                rs1,
                rs2,
                branch_target,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_7063, 8, 9, - 4096; "bgeu r8, r9, -4096")]
    #[test_case(0x7e94_7fe3, 8, 9, 4094; "bgeu r8, r9, 4094")]
    fn bgeu(word: u32, rs1: u8, rs2: u8, branch_target: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let branch_target = branch_target as u32;
        let match_ins = Instruction {
            op: Op::BGEU,
            args: Args {
                rs1,
                rs2,
                branch_target,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_f533, 10, 17, 18; "and r10, r17, r18")]
    fn and(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::AND,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff8_f513, 10, 17, 0xff; "andi r10, r17, 255")]
    fn andi(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::AND,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8008_c513, 10, 17, - 2048; "xori r10, r17, -2048")]
    fn xori(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::XOR,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_e533, 10, 17, 18; "or r10, r17, r18")]
    fn or(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::OR,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff8_e513, 10, 17, 0xff; "ori r10, r17, 255")]
    fn ori(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::OR,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_0023, 0, 10, - 2048; "sb r10, -2048(r0)")]
    #[test_case(0x7ea0_0fa3, 0, 10, 2047; "sb r10, 2047(r0)")]
    fn sb(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::SB,
            args: Args {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_1023, 0, 10, - 2048; "sh r10, -2048(r0)")]
    #[test_case(0x7ea0_1fa3, 0, 10, 2047; "sh r10, 2047(r0)")]
    fn sh(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::SH,
            args: Args {
                rs1,
                rs2,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_2023, 0, 10, - 2048; "sw r10, -2048(r0)")]
    #[test_case(0x7ea0_2fa3, 0, 10, 2047; "sw r10, 2047(r0)")]
    fn sw(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::SW,
            args: Args {
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
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::MUL,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_9533, 10, 17, 18; "mulh r10, r17, r18")]
    fn mulh(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::MULH,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_a533, 10, 17, 18; "mulhsu r10, r17, r18")]
    fn mulhsu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::MULHSU,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_b533, 10, 17, 18; "mulhu r10, r17, r18")]
    fn mulhu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::MULHU,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_af83, 31, 1, 2047; "lw r31, 2047(r1)")]
    #[test_case(0x8000_af83, 31, 1, - 2048; "lw r31, -2048(r1)")]
    fn lw(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LW,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_9f83, 31, 1, 2047; "lh r31, 2047(r1)")]
    #[test_case(0x8000_9f83, 31, 1, - 2048; "lh r31, -2048(r1)")]
    fn lh(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LH,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_df83, 31, 1, 2047; "lhu r31, 2047(r1)")]
    #[test_case(0x8000_df83, 31, 1, - 2048; "lhu r31, -2048(r1)")]
    fn lhu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LHU,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_8f83, 31, 1, 2047; "lb r31, 2047(r1)")]
    #[test_case(0x8000_8f83, 31, 1, - 2048; "lb r31, -2048(r1)")]
    fn lb(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LB,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_cf83, 31, 1, 2047; "lbu r31, 2047(r1)")]
    #[test_case(0x8000_cf83, 31, 1, - 2048; "lbu r31, -2048(r1)")]
    fn lbu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::LBU,
            args: Args {
                rd,
                rs1,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8000_00b7, 1, - 2_147_483_648; "lui r1, -524288")]
    #[test_case(0x7fff_f0b7, 1, 2_147_479_552; "lui r1, 524287")]
    fn lui(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::ADD,
            args: Args {
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8000_0097, 1, - 2_147_483_648; "auipc r1, -524288")]
    #[test_case(0x7fff_f097, 1, 2_147_479_552; "auipc r1, 524287")]
    fn auipc(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        let imm = imm as u32;
        let match_ins = Instruction {
            op: Op::ADD,
            args: Args {
                rd,
                imm,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_c533, 10, 17, 18; "div r10, r17, r18")]
    fn div(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::DIV,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_d533, 10, 17, 18; "divu r10, r17, r18")]
    fn divu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::DIVU,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_e533, 10, 17, 18; "rem r10, r17, r18")]
    fn rem(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::REM,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_f533, 10, 17, 18; "remu r10, r17, r18")]
    fn remu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::REMU,
            args: Args {
                rd,
                rs1,
                rs2,
                ..Default::default()
            },
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0000_0073; "ecall")]
    fn ecall(word: u32) {
        let ins: Instruction = decode_instruction(0, word);
        let match_ins = Instruction {
            op: Op::ECALL,
            args: Args::default(),
        };
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff0_000f, 0, 0, 255; "fence, iorw, iorw")]
    fn fence(word: u32, _rd: u8, _rs1: u8, _imm: i32) {
        let ins: Instruction = decode_instruction(0, word);
        assert_eq!(ins, NOOP);
    }

    #[test_case(0x3020_0073; "mret")]
    fn mret(word: u32) {
        let ins: Instruction = decode_instruction(0, word);
        assert_eq!(ins, NOOP);
    }

    #[test_case(0x3420_2f73, 30, 0, 834; "csrrs, t5, mcause")]
    fn csrrs(word: u32, _rd: u8, _rs1: u8, _imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        assert_eq!(ins, NOOP);
    }

    #[test_case(0x3052_9073, 0, 5, 773; "csrrw, mtvec, t0")]
    fn csrrw(word: u32, _rd: u8, _rs1: u8, _imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        assert_eq!(ins, NOOP);
    }

    #[test_case(0x7444_5073, 0, 8, 0x744; "csrrwi, 0x744, 8")]
    fn csrrwi(word: u32, _rd: u8, _rs1: u8, _imm: u32) {
        let ins: Instruction = decode_instruction(0, word);
        assert_eq!(ins, NOOP);
    }
}
