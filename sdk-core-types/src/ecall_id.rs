pub const HALT: u32 = 0;
pub const PANIC: u32 = 1;
pub const PRIVATE_TAPE: u32 = 2;
pub const POSEIDON2: u32 = 3;
pub const PUBLIC_TAPE: u32 = 4;
pub const CALL_TAPE: u32 = 5;
pub const EVENT_TAPE: u32 = 6;
pub const EVENTS_COMMITMENT_TAPE: u32 = 7;
pub const CAST_LIST_COMMITMENT_TAPE: u32 = 8;
pub const SELF_PROG_ID_TAPE: u32 = 9;
/// Syscall to output the VM trace log at `clk`. Useful for debugging.
pub const VM_TRACE_LOG: u32 = 10;
