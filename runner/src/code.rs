use std::collections::HashSet;

use anyhow::Result;
use derive_more::{Deref, IntoIterator};
use im::hashmap::HashMap;
use itertools::{chain, izip};
use mozak_sdk::core::ecall;
use plonky2::field::goldilocks_field::GoldilocksField;
use serde::{Deserialize, Serialize};

use crate::decode::{decode_instruction, ECALL};
use crate::elf::Program;
use crate::instruction::{Args, DecodingError, Instruction, Op};
use crate::state::{RawTapes, State};
use crate::vm::{step, ExecutionRecord};

/// Executable code of the ELF
///
/// A wrapper of a map from pc to [Instruction]
#[derive(Clone, Debug, Default, Deref, Serialize, Deserialize, PartialEq)]
pub struct Code(pub HashMap<u32, Result<Instruction, DecodingError>>);

impl Code {
    /// Get [Instruction] given `pc`
    #[must_use]
    pub fn get_instruction(&self, pc: u32) -> Option<&Result<Instruction, DecodingError>> {
        let Code(code) = self;
        code.get(&pc)
    }
}

impl From<&HashMap<u32, u8>> for Code {
    fn from(image: &HashMap<u32, u8>) -> Self {
        fn load_u32(m: &HashMap<u32, u8>, addr: u32) -> u32 {
            const WORD_SIZE: usize = 4;
            let mut bytes = [0_u8; WORD_SIZE];
            for (i, byte) in (addr..).zip(bytes.iter_mut()) {
                *byte = m.get(&i).copied().unwrap_or_default();
            }
            u32::from_le_bytes(bytes)
        }

        Self(
            image
                .keys()
                .map(|addr| addr & !3)
                .collect::<HashSet<_>>()
                .into_iter()
                .map(|key| (key, decode_instruction(key, load_u32(image, key))))
                .collect(),
        )
    }
}

#[must_use]
#[allow(clippy::similar_names)]
/// # Panics
///
/// Panics if the VM is not halted at its last state.
pub fn execute_code_with_ro_memory(
    code: impl IntoIterator<Item = Instruction>,
    ro_mem: &[(u32, u8)],
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
    raw_tapes: RawTapes,
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

    let program = Program::create(ro_mem, rw_mem, ro_code);
    let state0 = State::new(program.clone(), raw_tapes);

    let state = regs.iter().fold(state0, |state, (rs, val)| {
        state.set_register_value(*rs, *val)
    });

    let record = step(&program, state).unwrap();
    assert!(record.last_state.has_halted());
    (program, record)
}

/// Entrypoint for a stream of instructions into the VM.
///
/// Creates a [`Program`] and executes given
/// [Instruction]s
#[must_use]
pub fn execute(
    code: impl IntoIterator<Item = Instruction>,
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    execute_code_with_ro_memory(code, &[], rw_mem, regs, RawTapes::default())
}
