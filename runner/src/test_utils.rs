use im::hashmap::HashMap;
use plonky2::field::goldilocks_field::GoldilocksField;
#[cfg(any(feature = "test", test))]
use proptest::prelude::any;
#[cfg(any(feature = "test", test))]
use proptest::prop_oneof;
#[cfg(any(feature = "test", test))]
use proptest::strategy::{Just, Strategy};

use crate::elf::{Code, Data, Program};
use crate::instruction::{Args, Instruction, Op};
use crate::state::State;
use crate::system::ecall;
use crate::vm::{step, ExecutionRecord};

/// Returns the state just before the final state
#[must_use]
pub fn state_before_final(e: &ExecutionRecord<GoldilocksField>) -> &State<GoldilocksField> {
    &e.executed[e.executed.len() - 2].state
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::similar_names)]
pub fn simple_test_code_with_ro_memory(
    code: &[Instruction],
    ro_mem: &[(u32, u32)],
    rw_mem: &[(u32, u32)],
    regs: &[(u8, u32)],
    io_tape: &[u8],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    let _ = env_logger::try_init();
    let ro_code = Code(
        (0..)
            .step_by(4)
            .zip(
                code.iter()
                    .chain(
                        [
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
                                ..Default::default()
                            },
                        ]
                        .iter(),
                    )
                    .copied(),
            )
            .collect(),
    );

    let program = Program {
        ro_memory: Data::from(ro_mem.iter().copied().collect::<HashMap<u32, u32>>()),
        rw_memory: Data::from(rw_mem.iter().copied().collect::<HashMap<u32, u32>>()),
        ro_code,
        ..Default::default()
    };
    // TODO(Roman): fixme - extend function API to support public-io
    let state0 = State::new(program.clone(), io_tape, &[]);

    let state = regs.iter().fold(state0, |state, (rs, val)| {
        state.set_register_value(*rs, *val)
    });

    let record = step(&program, state).unwrap();
    assert!(record.last_state.has_halted());
    (program, record)
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn simple_test_code(
    code: &[Instruction],
    rw_mem: &[(u32, u32)],
    regs: &[(u8, u32)],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    simple_test_code_with_ro_memory(code, &[], rw_mem, regs, &[])
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn simple_test_code_with_io_tape(
    code: &[Instruction],
    rw_mem: &[(u32, u32)],
    regs: &[(u8, u32)],
    io_tapes: &[u8],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    simple_test_code_with_ro_memory(code, &[], rw_mem, regs, io_tapes)
}

#[cfg(any(feature = "test", test))]
#[allow(clippy::cast_sign_loss)]
pub fn u32_extra() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(0_u32),
        Just(1_u32),
        Just(u32::MAX),
        any::<u32>(),
        Just(i32::MIN as u32),
        Just(i32::MAX as u32),
    ]
}

#[cfg(any(feature = "test", test))]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn i32_extra() -> impl Strategy<Value = i32> { u32_extra().prop_map(|x| x as i32) }

#[cfg(any(feature = "test", test))]
#[allow(clippy::cast_possible_truncation)]
pub fn i16_extra() -> impl Strategy<Value = i16> { i32_extra().prop_map(|x| x as i16) }

#[cfg(any(feature = "test", test))]
#[allow(clippy::cast_possible_truncation)]
pub fn i8_extra() -> impl Strategy<Value = i8> { i32_extra().prop_map(|x| x as i8) }

#[cfg(any(feature = "test", test))]
#[allow(clippy::cast_possible_truncation)]
pub fn u16_extra() -> impl Strategy<Value = u16> { u32_extra().prop_map(|x| x as u16) }

#[cfg(any(feature = "test", test))]
#[allow(clippy::cast_possible_truncation)]
pub fn u8_extra() -> impl Strategy<Value = u8> { u32_extra().prop_map(|x| x as u8) }

#[cfg(any(feature = "test", test))]
pub fn reg() -> impl Strategy<Value = u8> { u8_extra().prop_map(|x| 1 + (x % 31)) }
