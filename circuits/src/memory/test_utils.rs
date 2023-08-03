use mozak_vm::elf::Program;
use mozak_vm::instruction::Op::{LB, SB};
use mozak_vm::instruction::{Args, Instruction};
use mozak_vm::test_utils::simple_test_code;
use mozak_vm::vm::{ExecutionRecord, Row};

/// # Panics
///
/// This function will panic if any of the following conditions are not met:
/// * The state loaded at address 100 is not equal to 10.
/// * The value of register 4 is not 5.
/// * The value of register 5 is not 10.
/// * The state loaded at address 200 is not equal to 15.
/// * The value of register 6 is not 15.
#[must_use]
pub fn memory_trace_test_case() -> (Program, Vec<Row>) {
    let new = Instruction::new;
    let (
        program,
        ExecutionRecord {
            executed,
            last_state: state,
        },
    ) = simple_test_code(
        &[
            new(SB, Args {
                rs2: 1,
                imm: 100,
                ..Args::default()
            }),
            new(LB, Args {
                rd: 4,
                imm: 100,
                ..Args::default()
            }),
            new(SB, Args {
                rs2: 3,
                imm: 200,
                ..Args::default()
            }),
            new(LB, Args {
                rd: 6,
                imm: 200,
                ..Args::default()
            }),
            new(SB, Args {
                rs2: 2,
                imm: 100,
                ..Args::default()
            }),
            new(LB, Args {
                rd: 5,
                imm: 100,
                ..Args::default()
            }),
        ],
        &[],
        &[(1, 5), (2, 10), (3, 15)],
    );

    assert_eq!(state.load_u8(100), 10);
    assert_eq!(state.get_register_value(4), 5);
    assert_eq!(state.get_register_value(5), 10);
    assert_eq!(state.load_u8(200), 15);
    assert_eq!(state.get_register_value(6), 15);
    (program, executed)
}
