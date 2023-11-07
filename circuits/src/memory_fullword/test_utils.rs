use mozak_runner::elf::Program;
use mozak_runner::instruction::Op::{LW, SW};
use mozak_runner::instruction::{Args, Instruction};
use mozak_runner::test_utils::simple_test_code;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::goldilocks_field::GoldilocksField;

// TODO(Matthias): Consider unifying with the byte memory example?
#[must_use]
pub fn fullword_memory_trace_test_case(
    repeats: usize,
) -> (Program, ExecutionRecord<GoldilocksField>) {
    let new = Instruction::new;
    let instructions = [
        new(SW, Args {
            // addr = rs2 + imm, value = rs1-value
            // store-full-word of address = 100, value 0x0a0b_0c0d
            rs1: 1,
            imm: 600,
            ..Args::default()
        }),
        new(LW, Args {
            // addr = rs2 + imm, value = rd-value
            // load-full-word from address = 100 to reg-3, value of 0x0a0b_0c0d
            rd: 3,
            imm: 600,
            ..Args::default()
        }),
        new(SW, Args {
            // addr = rs2 + imm, value = rs1
            // store-full-word of address = 200, value 0x0102_0304
            rs1: 2,
            imm: 700,
            ..Args::default()
        }),
        new(LW, Args {
            // addr = rs2 + imm, value = rd
            // load-full-word from address = 200 to reg-4, value of 0x0102_0304
            rd: 4,
            imm: 700,
            ..Args::default()
        }),
    ];
    let code = std::iter::repeat(&instructions)
        .take(repeats)
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let (program, record) = simple_test_code(
        &code,
        &[
            (600, 0),
            (601, 0),
            (602, 0),
            (603, 0),
            (700, 0),
            (701, 0),
            (702, 0),
            (703, 0),
        ],
        &[
            (1, 0x0a0b_0c0d),
            (2, 0x0102_0304),
            (3, 0xFFFF),
            (4, 0x0000_FFFF),
        ],
    );

    if repeats > 0 {
        let state = &record.last_state;
        assert_eq!(state.load_u32(600), 0x0a0b_0c0d);
        assert_eq!(state.get_register_value(3), 0x0a0b_0c0d);
        assert_eq!(state.load_u32(700), 0x0102_0304);
        assert_eq!(state.get_register_value(4), 0x0102_0304);
    }
    (program, record)
}
