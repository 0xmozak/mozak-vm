use mozak_vm::elf::Program;
use mozak_vm::instruction::Op::{LBU, SB};
use mozak_vm::instruction::{Args, Instruction};
use mozak_vm::test_utils::simple_test_code;
use mozak_vm::vm::ExecutionRecord;

#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn memory_trace_test_case() -> (Program, ExecutionRecord) {
    let new = Instruction::new;
    let (program, record) = simple_test_code(
        &[
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
        ],
        &[],
        &[(1, 255), (2, 10), (3, 15)],
    );

    let state = &record.last_state;
    assert_eq!(state.load_u8(100), 10);
    assert_eq!(state.get_register_value(4), 255);
    assert_eq!(state.get_register_value(5), 10);
    assert_eq!(state.load_u8(200), 15);
    assert_eq!(state.get_register_value(6), 15);
    (program, record)
}
