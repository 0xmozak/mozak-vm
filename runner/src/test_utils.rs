use itertools::{chain, izip};
use mozak_system::system::ecall;
use plonky2::field::goldilocks_field::GoldilocksField;
#[cfg(any(feature = "test", test))]
use proptest::prelude::any;
#[cfg(any(feature = "test", test))]
use proptest::prop_oneof;
#[cfg(any(feature = "test", test))]
use proptest::strategy::{Just, Strategy};

use crate::elf::{Code, Data, Program, RuntimeArguments};
use crate::instruction::{Args, Instruction, Op};
use crate::state::State;
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
    code: impl IntoIterator<Item = Instruction>,
    ro_mem: &[(u32, u8)],
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
    runtime_args: RuntimeArguments,
) -> (Program, ExecutionRecord<GoldilocksField>) {
    let RuntimeArguments {
        io_tape_private,
        io_tape_public,
        transcript,
        ..
    } = runtime_args;
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

    let state0 = State::new(program.clone(), crate::elf::RuntimeArguments {
        context_variables: vec![],
        io_tape_private,
        io_tape_public,
        transcript,
    });

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
    code: impl IntoIterator<Item = Instruction>,
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    simple_test_code_with_ro_memory(code, &[], rw_mem, regs, RuntimeArguments::default())
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn simple_test_code_with_runtime_args(
    code: impl IntoIterator<Item = Instruction>,
    rw_mem: &[(u32, u8)],
    regs: &[(u8, u32)],
    runtime_args: RuntimeArguments,
) -> (Program, ExecutionRecord<GoldilocksField>) {
    simple_test_code_with_ro_memory(code, &[], rw_mem, regs, runtime_args)
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
pub fn u64_extra() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0_u64),
        Just(1_u64),
        Just(u64::MAX),
        any::<u64>(),
        Just(i64::MIN as u64),
        Just(i64::MAX as u64),
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
