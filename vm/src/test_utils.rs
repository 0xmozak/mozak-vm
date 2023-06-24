use im::hashmap::HashMap;

use crate::elf::{Code, Data, Program};
use crate::instruction::{Args, Instruction, Op};
use crate::state::State;
use crate::vm::{step, ExecutionRecord};

#[must_use]
fn create_prog(image: HashMap<u32, u32>) -> State {
    State::from(Program::from(image))
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn simple_test_code(
    code: &[Instruction],
    mem: &[(u32, u32)],
    regs: &[(u8, u32)],
) -> ExecutionRecord {
    let _ = env_logger::try_init();
    let code = Code(
        (0..)
            .step_by(4)
            .zip(
                code.iter()
                    .chain(
                        [
                            // set sys-call EXIT in x17(or a7)
                            Instruction {
                                op: Op::ADD,
                                data: Args {
                                    rs1: 0,
                                    rs2: 0,
                                    rd: 17,
                                    imm: 93,
                                },
                            },
                            // add ECALL to halt the program
                            Instruction {
                                op: Op::ECALL,
                                data: Args {
                                    rs1: 0,
                                    rs2: 0,
                                    rd: 0,
                                    imm: 0,
                                },
                            },
                        ]
                        .iter(),
                    )
                    .copied(),
            )
            .collect(),
    );

    let image: HashMap<u32, u32> = mem.iter().copied().collect();
    let image = Data::from(image);
    let state0 = State::from(Program {
        entry: 0,
        data: image,
        code,
    });

    let state = regs.iter().fold(state0, |state, (rs, val)| {
        state.set_register_value(*rs, *val)
    });

    let record = step(state).unwrap();
    assert!(record.last_state.has_halted());
    record
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn simple_test(exit_at: u32, mem: &[(u32, u32)], regs: &[(u8, u32)]) -> ExecutionRecord {
    // TODO(Matthias): stick this line into proper common setup?
    let _ = env_logger::try_init();
    let exit_inst =
        // set sys-call EXIT in x17(or a7)
        &[(exit_at, 0x05d0_0893_u32),
        // add ECALL to halt the program
        (exit_at + 4, 0x0000_0073_u32)];

    let image: HashMap<u32, u32> = mem.iter().chain(exit_inst.iter()).copied().collect();

    let state = regs.iter().fold(create_prog(image), |state, (rs, val)| {
        state.set_register_value(*rs, *val)
    });

    let record = step(state).unwrap();
    assert!(record.last_state.has_halted());
    record
}
