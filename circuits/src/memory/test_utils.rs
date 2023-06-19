use mozak_vm::test_utils::simple_test;
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
pub fn memory_trace_test_case() -> Vec<Row> {
    // Store Byte: M[rs1 + imm] = rs2
    // imm[11:5]  rs2    rs1    funct3  imm[4:0]  opcode
    // Load Byte: rd = M[rs1 + imm]
    // imm[11:0]         rs1    funct3  rd        opcode
    // 0000011    00001  00000  000     00100     0100011   sb r1, 100(r0)
    // 000001100100      00000  000     00100     0000011   lb r4, 100(r0)
    // 0000110    00011  00000  000     01000     0100011   sb r3, 200(r0)
    // 000011001000      00000  000     00110     0000011   lb r6, 200(r0)
    // 0000011    00010  00000  000     00100     0100011   sb r2, 100(r0)
    // 000001100100      00000  000     00101     0000011   lb r5, 100(r0)
    let (exit_at, mem, reg) = (
        24,
        [
            (0_u32, 0b0000_0110_0001_0000_0000_0010_0010_0011),
            (4_u32, 0b0000_0110_0100_0000_0000_0010_0000_0011),
            (8_u32, 0b0000_1100_0011_0000_0000_0100_0010_0011),
            (12_u32, 0b0000_1100_1000_0000_0000_0011_0000_0011),
            (16_u32, 0b0000_0110_0010_0000_0000_0010_0010_0011),
            (20_u32, 0b0000_0110_0100_0000_0000_0010_1000_0011),
        ],
        [(1, 5), (2, 10), (3, 15)],
    );

    let ExecutionRecord {
        executed,
        last_state: state,
    } = simple_test(exit_at, &mem, &reg);
    assert_eq!(state.load_u8(100), 10);
    assert_eq!(state.get_register_value(4), 5);
    assert_eq!(state.get_register_value(5), 10);
    assert_eq!(state.load_u8(200), 15);
    assert_eq!(state.get_register_value(6), 15);
    executed
}
