use mozak_runner::elf::Program;
use mozak_runner::instruction::Op::{LH, LHU, SH};
use mozak_runner::instruction::{Args, Instruction};
use mozak_runner::test_utils::simple_test_code;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::goldilocks_field::GoldilocksField;

// TODO(Matthias): Consider unifying with the byte memory example?
#[must_use]
pub fn halfword_memory_trace_test_case(
    repeats: usize,
) -> (Program, ExecutionRecord<GoldilocksField>) {
    let new = Instruction::new;
    let instructions = [
        new(SH, Args {
            // addr = rs2 + imm, value = rs1-value
            // store-full-word of address = 100, value 0x0102
            rs1: 1,
            imm: 400,
            ..Args::default()
        }),
        new(LH, Args {
            // addr = rs2 + imm, value = rd-value
            // load-full-word from address = 100 to reg-3, value of 0x0102
            rd: 3,
            imm: 400,
            ..Args::default()
        }),
        new(SH, Args {
            // addr = rs2 + imm, value = rs1
            // store-full-word of address = 200, value 0x0304
            rs1: 2,
            imm: 500,
            ..Args::default()
        }),
        new(LHU, Args {
            // addr = rs2 + imm, value = rd
            // load-full-word from address = 200 to reg-4, value of 0x0304
            rd: 4,
            imm: 500,
            ..Args::default()
        }),
    ];
    let code = std::iter::repeat(&instructions)
        .take(repeats)
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let (program, record) = simple_test_code(
        code,
        &[
            (400, 0),
            (401, 0),
            (402, 0),
            (403, 0),
            (500, 0),
            (501, 0),
            (502, 0),
        ],
        &[(1, 0x0102), (2, 0x0304), (3, 0xFFFF), (4, 0x0000_FFFF)],
    );

    if repeats > 0 {
        let state = &record.last_state;
        assert_eq!(state.load_u32(400), 0x0102);
        assert_eq!(state.get_register_value(3), 0x0102);
        assert_eq!(state.load_u32(500), 0x0304);
        assert_eq!(state.get_register_value(4), 0x0304);
    }
    (program, record)
}
