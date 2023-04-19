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
