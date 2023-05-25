use bitfield::bitfield;
use bitfield::BitRange;

use crate::instruction::{
    BTypeInst, ITypeInst, Instruction, JTypeInst, RTypeInst, STypeInst, UTypeInst,
};

/// Builds a i32 from segments, like [(0, 4), (20, 32)], shifts left afterwards
///
/// The shift is built-in for convenience, because the type annotation syntax in
/// Rust is a bit awkward otherwise.
#[must_use]
fn extract_immediate(word: u32, segments: &[(usize, usize)], shift: usize) -> i32 {
    let len: usize = segments.iter().map(|(msb, lsb)| msb - lsb + 1).sum();
    let u = segments.iter().fold(0, |acc, (msb, lsb)| -> u32 {
        let bits: u32 = word.bit_range(*msb, *lsb);
        (acc << (msb - lsb + 1)) | bits
    });
    let bit_size = std::mem::size_of::<u32>() * 8;
    // shift back and forth for sign extension.
    ((u << (bit_size - len)) as i32) >> (bit_size - len - shift)
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

    let stype = STypeInst {
        rs1,
        rs2,
        imm: extract_immediate(word, &[(31, 31), (30, 25), (11, 8), (7, 7)], 0),
    };
    let rtype = RTypeInst { rs1, rs2, rd };
    let itype = ITypeInst {
        rs1,
        rd,
        imm: extract_immediate(word, &[(31, 20)], 0),
    };
    let jtype = JTypeInst {
        rd,
        imm: extract_immediate(word, &[(31, 31), (19, 12), (20, 20), (30, 25), (24, 21)], 1),
    };
    let btype = BTypeInst {
        rs1,
        rs2,
        imm: extract_immediate(word, &[(31, 31), (7, 7), (30, 25), (11, 8)], 1),
    };
    let utype = UTypeInst {
        rd,
        imm: extract_immediate(word, &[(31, 12)], 12),
    };
    match bf.opcode() {
        0b011_0011 => match (bf.func3(), bf.func7()) {
            (0x0, 0x00) => Instruction::ADD(rtype),
            (0x0, 0x20) => Instruction::SUB(rtype),
            (0x1, 0x00) => Instruction::SLL(rtype),
            (0x2, 0x00) => Instruction::SLT(rtype),
            (0x3, 0x00) => Instruction::SLTU(rtype),
            (0x4, 0x00) => Instruction::XOR(rtype),
            (0x5, 0x00) => Instruction::SRL(rtype),
            (0x5, 0x20) => Instruction::SRA(rtype),
            (0x6, 0x00) => Instruction::OR(rtype),
            (0x7, 0x00) => Instruction::AND(rtype),
            (0x4, 0x01) => Instruction::DIV(rtype),
            (0x5, 0x01) => Instruction::DIVU(rtype),
            (0x6, 0x01) => Instruction::REM(rtype),
            (0x7, 0x01) => Instruction::REMU(rtype),
            (0x0, 0x01) => Instruction::MUL(rtype),
            (0x1, 0x01) => Instruction::MULH(rtype),
            (0x2, 0x01) => Instruction::MULHSU(rtype),
            (0x3, 0x01) => Instruction::MULHU(rtype),
            _ => Instruction::UNKNOWN,
        },
        0b000_0011 => match bf.func3() {
            0x0 => Instruction::LB(itype),
            0x1 => Instruction::LH(itype),
            0x2 => Instruction::LW(itype),
            0x4 => Instruction::LBU(itype),
            0x5 => Instruction::LHU(itype),
            _ => Instruction::UNKNOWN,
        },
        0b010_0011 => match bf.func3() {
            0x0 => Instruction::SB(stype),
            0x1 => Instruction::SH(stype),
            0x2 => Instruction::SW(stype),
            _ => Instruction::UNKNOWN,
        },
        0b001_0011 => match bf.func3() {
            0x0 => Instruction::ADDI(itype),
            0x1 => Instruction::SLLI(itype),
            0x2 => Instruction::SLTI(itype),
            0x3 => Instruction::SLTIU(itype),
            0x4 => Instruction::XORI(itype),
            0x5 => {
                let imm = itype.imm as u32;
                let imm_masked: u32 = imm.bit_range(4, 0);
                let itype = ITypeInst {
                    imm: imm_masked as i32,
                    ..itype
                };
                // Masks the first 7 bits in a word to differentiate between an
                // SRAI/SRLI instruction. They have the same funct3 value and are
                // differentiated by their 30th bit, for which SRAI = 1 and SRLI = 0.
                match imm.bit_range(11, 5) {
                    0b0100000 => Instruction::SRAI(itype),
                    0 => Instruction::SRLI(itype),
                    _ => Instruction::UNKNOWN,
                }
            }
            0x6 => Instruction::ORI(itype),
            0x7 => Instruction::ANDI(itype),
            _ => Instruction::UNKNOWN,
        },
        0b111_0011 => match bf.func12() {
            0x0 => Instruction::ECALL,
            0x1 => Instruction::EBREAK,
            _ => Instruction::UNKNOWN,
        },
        0b110_1111 => Instruction::JAL(jtype),
        0b110_0111 => match bf.func3() {
            0x0 => Instruction::JALR(itype),
            _ => Instruction::UNKNOWN,
        },
        0b110_0011 => match bf.func3() {
            0x0 => Instruction::BEQ(btype),
            0x1 => Instruction::BNE(btype),
            0x4 => Instruction::BLT(btype),
            0x5 => Instruction::BGE(btype),
            0x6 => Instruction::BLTU(btype),
            0x7 => Instruction::BGEU(btype),
            _ => Instruction::UNKNOWN,
        },
        0b011_0111 => Instruction::LUI(utype),
        0b001_0111 => Instruction::AUIPC(utype),
        _ => Instruction::UNKNOWN,
    }
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use super::{decode_instruction, extract_immediate};
    use crate::instruction::{
        BTypeInst, ITypeInst, Instruction, JTypeInst, RTypeInst, STypeInst, UTypeInst,
    };

    #[test_case(0b000_1100, 3; "extract 3")]
    #[test_case(0b1101_1100, -1; "extract neg 3")]
    fn extract_simple(word: u32, x: i32) {
        let a: i32 = extract_immediate(word, &[(7, 6), (4, 2)], 0);
        assert_eq!(x, a);
    }
    #[test_case(0x018B_80B3, 1, 23, 24; "add r1, r23, r24")]
    #[test_case(0x0000_0033, 0, 0, 0; "add r0, r0, r0")]
    #[test_case(0x01FF_8FB3, 31, 31, 31; "add r31, r31, r31")]
    fn add(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::ADD(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff1_8193, 3, 3, 2047; "addi r3, r3, 2047")]
    #[test_case(0x8001_8193, 3, 3, -2048; "addi r3, r3, -2048")]
    #[test_case(0x4480_0f93, 31, 0, 1096; "addi r31, r0, 1096")]
    #[test_case(0xdca5_8e13, 28, 11, -566; "addi r28, r11, -566")]
    fn addi(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::ADDI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_92b3, 5, 17, 18; "sll r5, r17, r18")]
    fn sll(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLL(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f2_1213, 4, 4, 31; "slli r4, r4, 31")]
    #[test_case(0x0076_9693, 13, 13, 7; "slli r13, r13, 7")]
    fn slli(word: u32, rd: u8, rs1: u8, shamt: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLLI(ITypeInst {
            rs1,
            rd,
            imm: shamt.into(),
        });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_52b3, 5, 18, 19; "srl r5, r18, r19")]
    fn srl(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SRL(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x4139_52b3, 5, 18, 19; "sra r5, r18, r19")]
    fn sra(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SRA(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_22b3, 5, 18, 19; "slt r5, r18, r19")]
    fn slt(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLT(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x41f9_5293, 5, 18, 31; "srai r5, r18, 31")]
    fn srai(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SRAI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x01f9_5293, 5, 18, 31; "srli r5, r18, 31")]
    fn srli(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SRLI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff9_2293, 5, 18, 255; "slti r5, r18, 255")]
    fn slti(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLTI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff9_3293, 5, 18, 255; "sltiu r5, r18, 255")]
    fn sltiu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLTIU(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0139_32b3, 5, 18, 19; "sltu r5, r18, r19")]
    fn sltu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SLTU(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x4094_01b3, 3, 8, 9; "sub r3, r8, r9")]
    #[test_case(0x4073_83b3, 7, 7, 7; "sub r7, r7, r7")]
    #[test_case(0x41bc_8733, 14, 25, 27; "sub r14, r25, r27")]
    fn sub(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SUB(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8400_00ef,1, -1_048_512; "jal r1, -1048512")]
    #[test_case(0x7c1f_fa6f,20, 1_048_512; "jal r20, 1048512")]
    fn jal(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::JAL(JTypeInst { rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff8_8567,10, 17, 2047; "jalr r10, r17, 2047")]
    #[test_case(0x8005_8ae7,21, 11, -2048; "jalr r21, r11, -2048")]
    fn jalr(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::JALR(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_1063,8, 9, -4096; "bne r8, r9, -4096")]
    #[test_case(0x7e94_1fe3,8, 9, 4094; "bne r8, r9, 4094")]
    fn bne(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BNE(BTypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_0063,8, 9, -4096; "beq r8, r9, -4096")]
    #[test_case(0x7e94_0fe3,8, 9, 4094; "beq r8, r9, 4094")]
    fn beq(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BEQ(BTypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_4063,8, 9, -4096; "blt r8, r9, -4096")]
    #[test_case(0x7e94_4fe3,8, 9, 4094; "blt r8, r9, 4094")]
    fn blt(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BLT(BTypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_6063,8, 9, -4096; "bltu r8, r9, -4096")]
    #[test_case(0x7e94_6fe3,8, 9, 4094; "bltu r8, r9, 4094")]
    fn bltu(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BLTU(BTypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_5063,8, 9, -4096; "bge r8, r9, -4096")]
    #[test_case(0x7e94_5fe3,8, 9, 4094; "bge r8, r9, 4094")]
    fn bge(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BGE(BTypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8094_7063,8, 9, -4096; "bgeu r8, r9, -4096")]
    #[test_case(0x7e94_7fe3,8, 9, 4094; "bgeu r8, r9, 4094")]
    fn bgeu(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::BGEU(BTypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_f533, 10, 17, 18; "and r10, r17, r18")]
    fn and(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::AND(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff8_f513, 10, 17, 0xff; "andi r10, r17, 255")]
    fn andi(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::ANDI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8008_c513, 10, 17, -2048; "xori r10, r17, -2048")]
    fn xori(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::XORI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0128_e533, 10, 17, 18; "or r10, r17, r18")]
    fn or(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::OR(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0ff8_e513, 10, 17, 0xff; "ori r10, r17, 255")]
    fn ori(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::ORI(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_0023, 0, 10, -2048; "sb r10, -2048(r0)")]
    #[test_case(0x7ea0_0fa3, 0, 10, 2047; "sb r10, 2047(r0)")]
    fn sb(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SB(STypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_1023, 0, 10, -2048; "sh r10, -2048(r0)")]
    #[test_case(0x7ea0_1fa3, 0, 10, 2047; "sh r10, 2047(r0)")]
    fn sh(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SH(STypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x80a0_2023, 0, 10, -2048; "sw r10, -2048(r0)")]
    #[test_case(0x7ea0_2fa3, 0, 10, 2047; "sw r10, 2047(r0)")]
    fn sw(word: u32, rs1: u8, rs2: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::SW(STypeInst { rs1, rs2, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_8533, 10, 17, 18; "mul r10, r17, r18")]
    fn mul(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::MUL(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_9533, 10, 17, 18; "mulh r10, r17, r18")]
    fn mulh(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::MULH(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_a533, 10, 17, 18; "mulhsu r10, r17, r18")]
    fn mulhsu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::MULHSU(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_b533, 10, 17, 18; "mulhu r10, r17, r18")]
    fn mulhu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::MULHU(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_af83, 31, 1, 2047; "lw r31, 2047(r1)")]
    #[test_case(0x8000_af83, 31, 1, -2048; "lw r31, -2048(r1)")]
    fn lw(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LW(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_9f83, 31, 1, 2047; "lh r31, 2047(r1)")]
    #[test_case(0x8000_9f83, 31, 1, -2048; "lh r31, -2048(r1)")]
    fn lh(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LH(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_df83, 31, 1, 2047; "lhu r31, 2047(r1)")]
    #[test_case(0x8000_df83, 31, 1, -2048; "lhu r31, -2048(r1)")]
    fn lhu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LHU(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_8f83, 31, 1, 2047; "lb r31, 2047(r1)")]
    #[test_case(0x8000_8f83, 31, 1, -2048; "lb r31, -2048(r1)")]
    fn lb(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LB(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x7ff0_cf83, 31, 1, 2047; "lbu r31, 2047(r1)")]
    #[test_case(0x8000_cf83, 31, 1, -2048; "lbu r31, -2048(r1)")]
    fn lbu(word: u32, rd: u8, rs1: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LBU(ITypeInst { rs1, rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8000_00b7, 1, -2_147_483_648; "lui r1, -524288")]
    #[test_case(0x7fff_f0b7, 1, 2_147_479_552; "lui r1, 524287")]
    fn lui(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::LUI(UTypeInst { rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x8000_0097, 1, -2_147_483_648; "auipc r1, -524288")]
    #[test_case(0x7fff_f097, 1, 2_147_479_552; "auipc r1, 524287")]
    fn auipc(word: u32, rd: u8, imm: i32) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::AUIPC(UTypeInst { rd, imm });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_c533, 10, 17, 18; "div r10, r17, r18")]
    fn div(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::DIV(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_d533, 10, 17, 18; "divu r10, r17, r18")]
    fn divu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::DIVU(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_e533, 10, 17, 18; "rem r10, r17, r18")]
    fn rem(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::REM(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }

    #[test_case(0x0328_f533, 10, 17, 18; "remu r10, r17, r18")]
    fn remu(word: u32, rd: u8, rs1: u8, rs2: u8) {
        let ins: Instruction = decode_instruction(word);
        let match_ins = Instruction::REMU(RTypeInst { rs1, rs2, rd });
        assert_eq!(ins, match_ins);
    }
}
