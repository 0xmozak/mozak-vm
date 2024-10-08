use mozak_runner::code;
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op::{LBU, SB};
use mozak_runner::instruction::{Args, Instruction};
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::goldilocks_field::GoldilocksField;

#[must_use]
pub fn memory_trace_test_case(repeats: usize) -> (Program, ExecutionRecord<GoldilocksField>) {
    let new = Instruction::new;
    let instructions = [
        new(SB, Args {
            rs1: 1,
            imm: 100,
            ..Args::default()
        }),
        new(LBU, Args {
            rd: 4,
            imm: 100,
            ..Args::default()
        }),
        new(SB, Args {
            rs1: 3,
            imm: 200,
            ..Args::default()
        }),
        new(LBU, Args {
            rd: 6,
            imm: 200,
            ..Args::default()
        }),
        new(SB, Args {
            rs1: 2,
            imm: 100,
            ..Args::default()
        }),
        new(LBU, Args {
            rd: 5,
            imm: 100,
            ..Args::default()
        }),
    ];
    let code = std::iter::repeat(&instructions)
        .take(repeats)
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let (program, record) = code::execute(
        code,
        &[(101, 0), (102, 0), (103, 0), (201, 0), (202, 0), (203, 0)],
        &[(1, 255), (2, 10), (3, 15)],
    );

    if repeats > 0 {
        let state = &record.last_state;
        assert_eq!(state.load_u8(100), 10);
        assert_eq!(state.get_register_value(4), 255);
        assert_eq!(state.get_register_value(5), 10);
        assert_eq!(state.load_u8(200), 15);
        assert_eq!(state.get_register_value(6), 15);
    }
    (program, record)
}
