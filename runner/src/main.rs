use mozak_runner::decode::decode_instruction;
use mozak_runner::instruction::{Args, Instruction, Op};

fn main() {
    let instruction = decode_instruction(0, 0x018B_80B3);

    assert!(
        instruction
            == Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 1,
                    rs1: 23,
                    rs2: 24,
                    imm: 0,
                }
            }
    );
}
