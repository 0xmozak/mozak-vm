use anyhow::Result;
use im::hashmap::HashMap;

use crate::elf::{Code, Program};
use crate::state::State;
use crate::vm::{step, Row};

impl State {
    pub fn set_register_value_mut(&mut self, index: usize, value: u32) {
        *self = self.clone().set_register_value(index, value);
    }

    pub fn set_pc_mut(&mut self, value: u32) {
        *self = self.clone().set_pc(value);
    }

    #[must_use]
    pub fn get_register_value_signed(&self, index: usize) -> i32 {
        self.get_register_value(index) as i32
    }

    /// Store a word to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    pub fn store_u32(&mut self, addr: u32, value: u32) -> Result<()> {
        let bytes = value.to_le_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            *self = self.clone().store_u8(addr + i as u32, *byte);
        }
        Ok(())
    }

    /// Load a halfword from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    #[must_use]
    pub fn load_u16(&self, addr: u32) -> u16 {
        let mut bytes = [0_u8; 2];
        bytes[0] = self.load_u8(addr);
        bytes[1] = self.load_u8(addr + 1_u32);
        u16::from_le_bytes(bytes)
    }
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
