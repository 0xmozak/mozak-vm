use im::hashmap::HashMap;
use itertools::{chain, izip};
use mozak_sdk::core::ecall;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::decode::ECALL;
use crate::elf::{Code, Program, RuntimeArguments};
use crate::instruction::{Args, Instruction, Op};
use crate::state::State;
use crate::vm::{step, ExecutionRecord};

#[must_use]
pub fn load_u32(m: &HashMap<u32, u8>, addr: u32) -> u32 {
    const WORD_SIZE: usize = 4;
    let mut bytes = [0_u8; WORD_SIZE];
    for (i, byte) in (addr..).zip(bytes.iter_mut()) {
        *byte = m.get(&i).copied().unwrap_or_default();
    }
    u32::from_le_bytes(bytes)
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::similar_names)]
// TODO(Roman): refactor this later (runtime_args)
#[allow(clippy::needless_pass_by_value)]
pub fn execute_code_with_ro_memory(
    code: impl IntoIterator<Item = Instruction>,
    ro_mem: &[(u32, u8)],
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
    runtime_args: RuntimeArguments,
) -> (Program, ExecutionRecord<GoldilocksField>) {
    let _ = env_logger::try_init();
    let ro_code = Code(
        izip!(
            (0..).step_by(4),
            chain!(code, [
                // set sys-call HALT in x10(or a0)
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: 10,
                        imm: ecall::HALT,
                        ..Args::default()
                    },
                },
                // add ECALL to halt the program
                ECALL,
            ])
            .map(Ok),
        )
        .collect(),
    );

    #[cfg(any(feature = "test", test))]
    let program = Program::create(ro_mem, rw_mem, &ro_code, &runtime_args);
    #[cfg(not(any(feature = "test", test)))]
    let program = Program::create_with_args(ro_mem, rw_mem, &ro_code, &runtime_args);
    let state0 = State::new(program.clone());

    let state = regs.iter().fold(state0, |state, (rs, val)| {
        state.set_register_value(*rs, *val)
    });

    let record = step(&program, state).unwrap();
    assert!(record.last_state.has_halted());
    (program, record)
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn execute_code(
    code: impl IntoIterator<Item = Instruction>,
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    execute_code_with_ro_memory(code, &[], rw_mem, regs, RuntimeArguments::default())
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn execute_code_with_runtime_args(
    code: impl IntoIterator<Item = Instruction>,
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
    runtime_args: RuntimeArguments,
) -> (Program, ExecutionRecord<GoldilocksField>) {
    execute_code_with_ro_memory(code, &[], rw_mem, regs, runtime_args)
}
