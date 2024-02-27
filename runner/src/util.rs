use im::hashmap::HashMap;
use itertools::{chain, izip};
use mozak_system::system::ecall;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::elf::{Code, Data, Program, RuntimeArguments};
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
                Instruction {
                    op: Op::ECALL,
                    args: Args::default(),
                },
            ])
            .map(Ok),
        )
        .collect(),
    );

    let program = Program {
        ro_memory: Data(ro_mem.iter().copied().collect()),
        rw_memory: Data(rw_mem.iter().copied().collect()),
        ro_code,
        ..Default::default()
    };

    // let RuntimeArguments {
    //     self_prog_id,
    //     cast_list,
    //     io_tape_private,
    //     io_tape_public,
    //     call_tape,
    //     event_tape,
    // } = runtime_args;

    let state0 = if runtime_args.is_empty() {
        State::new(program.clone(), runtime_args)
    } else {
        State::legacy_ecall_api_new(program.clone(), runtime_args)
    };

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
