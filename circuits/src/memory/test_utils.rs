use mozak_vm::elf::Program;
use mozak_vm::instruction::Op::{LBU, SB};
use mozak_vm::instruction::{Args, Instruction};
use mozak_vm::test_utils::simple_test_code;
use mozak_vm::vm::ExecutionRecord;

#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn memory_trace_test_case(repeats: usize) -> (Program, ExecutionRecord) {
    assert!(
        repeats < 416,
        "test case may infringe on read-only section (code) for SB operations"
    );
    let new = Instruction::new;
    let instructions = [
        new(SB, Args {
            rs1: 1,
            imm: 10000,
            ..Args::default()
        }),
        new(LBU, Args {
            rd: 4,
            imm: 10000,
            ..Args::default()
        }),
        new(SB, Args {
            rs1: 3,
            imm: 20000,
            ..Args::default()
        }),
        new(LBU, Args {
            rd: 6,
            imm: 20000,
            ..Args::default()
        }),
        new(SB, Args {
            rs1: 2,
            imm: 10000,
            ..Args::default()
        }),
        new(LBU, Args {
            rd: 5,
            imm: 10000,
            ..Args::default()
        }),
    ];
    let code = std::iter::repeat(&instructions)
        .take(repeats)
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let (program, record) = simple_test_code(&code, &[], &[(1, 255), (2, 10), (3, 15)]);

    if repeats > 0 {
        let state = &record.last_state;
        assert_eq!(state.load_u8(10000), 10);
        assert_eq!(state.get_register_value(4), 255);
        assert_eq!(state.get_register_value(5), 10);
        assert_eq!(state.load_u8(20000), 15);
        assert_eq!(state.get_register_value(6), 15);
    }
    (program, record)
}
