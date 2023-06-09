use anyhow::Result;
use im::hashmap::HashMap;

use crate::elf::{Code, Program};
use crate::state::State;
use crate::vm::{step, Row};

impl State {

}

impl From<HashMap<u32, u32>> for Program {
    fn from(image: HashMap<u32, u32>) -> Self {
        let image = image
            .iter()
            .flat_map(move |(k, v)| {
                v.to_le_bytes()
                    .into_iter()
                    .enumerate()
                    .map(move |(i, b)| (k + i as u32, b))
            })
            .collect();
        Self {
            entry: 0_u32,
            code: Code::from(&image),
            image,
        }
    }
}

fn create_prog(image: HashMap<u32, u32>) -> State {
    State::from(Program::from(image))
}

#[must_use]
pub fn simple_test(exit_at: u32, mem: &[(u32, u32)], regs: &[(usize, u32)]) -> (Vec<Row>, State) {
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

    let (state_rows, state) = step(state).unwrap();
    assert!(state.has_halted());
    (state_rows, state)
}
