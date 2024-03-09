use itertools::{chain, Itertools};
use mozak_runner::instruction::Args;
use mozak_runner::state::State;
use mozak_runner::vm::ExecutionRecord;
use mozak_system::system::reg_abi::REG_A1;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::register::columns::{dummy, init, read, write, Ops, Register};

/// Sort rows into blocks of ascending addresses, and then sort each block
/// internally by `augmented_clk`
#[must_use]
pub fn sort_into_address_blocks<F: RichField>(mut trace: Vec<Register<F>>) -> Vec<Register<F>> {
    trace.sort_by_key(|row| {
        (
            row.addr.to_noncanonical_u64(),
            row.clk.to_noncanonical_u64(),
            1 - row.ops.is_init.to_noncanonical_u64(),
            1 - row.ops.is_read.to_noncanonical_u64(),
            1 - row.ops.is_write.to_noncanonical_u64(),
        )
    });
    trace
}

fn init_register_trace<F: RichField>(state: &State<F>) -> Vec<Register<F>> {
    (1..32)
        .map(|i| Register {
            addr: F::from_canonical_u8(i),
            ops: init(),
            value: F::from_canonical_u32(state.get_register_value(i)),
            ..Default::default()
        })
        .collect()
}

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<Register<F>>) -> Vec<Register<F>> {
    let len = trace.len().next_power_of_two().max(MIN_TRACE_LENGTH);
    trace.resize(len, Register {
        ops: dummy(),
        // ..And fill other columns with duplicate of last real trace row.
        ..*trace.last().unwrap()
    });
    trace
}

/// Generates the trace for registers.
///
/// There are 3 steps:
/// 1) populate the trace with a similar layout as the
/// [`RegisterInit` table](crate::registerinit::columns),
/// 2) go through the program and extract all ops that act on registers,
/// filling up this table,
/// 3) pad with dummy rows (`is_used` == 0) to ensure that trace is a power of
///    2.
#[must_use]
pub fn generate_register_trace<F: RichField>(record: &ExecutionRecord<F>) -> Vec<Register<F>> {
    let ExecutionRecord {
        executed,
        last_state,
    } = record;

    let build_single_register_trace_row = |reg: fn(&Args) -> u8, ops: Ops<F>| -> _ {
        executed
            .iter()
            .filter(move |row| reg(&row.instruction.args) != 0)
            .map(move |row| {
                let reg = reg(&row.instruction.args);

                // Ignore r0 because r0 should always be 0.
                // TODO: assert r0 = 0 constraint in CPU trace.
                Register {
                    addr: F::from_canonical_u8(reg),
                    value: F::from_canonical_u32(if ops.is_write.is_one() {
                        row.aux.dst_val
                    } else {
                        row.state.get_register_value(reg)
                    }),
                    clk: F::from_canonical_u64(row.state.clk),
                    ops,
                    ..Default::default()
                }
            })
    };
    // TODO: see about deduplicating code with `build_single_register_trace_row`.
    let build_ecall_io_register_trace_row = || -> _ {
        executed.iter().filter_map(move |row| {
            let io = row.aux.io.as_ref()?;
            Some(Register {
                addr: F::from_canonical_u8(REG_A1),
                value: F::from_canonical_u32(io.addr),
                clk: F::from_canonical_u64(row.state.clk),
                ops: read(),
                ..Default::default()
            })
        })
    };
    let trace = sort_into_address_blocks(
        chain!(
            init_register_trace(record.executed.first().map_or(last_state, |row| &row.state)),
            build_ecall_io_register_trace_row(),
            // TODO: give both reads the same offset, so we have potentially fewer rows at higher
            // multiplicity.
            // Oh, perhaps just build the augmented clk out of normal clk * 2 plus ops?
            // DONE!  But we still need to merge repeated rows into a higher multiplicity one.
            // Maybe.
            build_single_register_trace_row(|Args { rs1, .. }| *rs1, read()),
            build_single_register_trace_row(|Args { rs2, .. }| *rs2, read()),
            build_single_register_trace_row(|Args { rd, .. }| *rd, write())
        )
        .collect_vec(),
    );
    log::trace!("trace {:?}", trace);

    pad_trace(trace)
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use mozak_runner::elf::Program;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::util::execute_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::test_utils::prep_table;

    type F = GoldilocksField;

    fn setup() -> (Program, ExecutionRecord<F>) {
        // Use same instructions as in the Notion document, see:
        // https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2?pvs=4#0729f89ddc724967ac991c9e299cc4fc
        let instructions = [
            Instruction::new(Op::ADD, Args {
                rs1: 6,
                rs2: 7,
                rd: 4,
                ..Args::default()
            }),
            Instruction::new(Op::ADD, Args {
                rs1: 4,
                rs2: 6,
                rd: 5,
                ..Args::default()
            }),
            Instruction::new(Op::ADD, Args {
                rs1: 5,
                rd: 4,
                imm: 100,
                ..Args::default()
            }),
        ];

        execute_code(instructions, &[], &[(6, 100), (7, 200)])
    }

    #[rustfmt::skip]
    fn expected_trace_initial<F: RichField>() -> Vec<Register<F>> {
        prep_table(
            (1..32)
                .map(|i| {
                    let value = match i {
                        6 => 100,
                        7 => 200,
                        _ => 0,
                    };
                    // Columns (repeated for registers 0-31):
                    // addr  value augmented_clk  diff_augmented_clk  is_init is_read is_write
                    [     i, value,            0,                           1,      0,      0]
                })
                .collect_vec(),
        )
    }

    #[test]
    fn generate_reg_trace_initial() {
        let (_, record) = setup();
        let trace = init_register_trace::<F>(&record.executed[0].state);
        let expected_trace_initial = expected_trace_initial();
        (0..31).for_each(|i| {
            assert_eq!(
                trace[i], expected_trace_initial[i],
                "Initial trace setup is wrong at row {i}"
            );
        });
    }

    #[test]
    #[rustfmt::skip]
    fn generate_reg_trace() {
        let (_program, record) = setup();

        // TODO: generate this from cpu rows?
        // For now, use program and record directly to avoid changing the CPU columns
        // yet.
        let trace = generate_register_trace::<F>(&record);

        // This is the actual trace of the instructions.
        let mut expected_trace = prep_table(
            vec![
                // First, populate the table with the instructions from the above test code.
                // Note that we filter out operations that act on r0.
                //
                // Columns:
                // addr value           clk  diff_augmented_clk  is_init is_read is_write
                [    1,    0,             0,                           1,      0,       0], // init
                [    2,    0,             0,                           1,      0,       0], // init
                [    3,    0,             0,                           1,      0,       0], // init
                [    4,    0,             0,                           1,      0,       0], // init
                [    4,  300,             2,                           0,      0,       1], // 1st inst
                [    4,  300,             3,                           0,      1,       0], // 2nd inst
                [    4,  500,             4,                           0,      0,       1], // 3rd inst
                [    5,    0,             0,                           1,      0,       0], // init
                [    5,  400,             3,                           0,      0,       1], // 2nd inst
                [    5,  400,             4,                           0,      1,       0], // 3rd inst
                [    6,  100,             0,                           1,      0,       0], // init
                [    6,  100,             2,                           0,      1,       0], // 1st inst
                [    6,  100,             3,                           0,      1,       0], // 2nd inst
                [    7,  200,             0,                           1,      0,       0], // init
                [    7,  200,             2,                           0,      1,       0], // 1st inst
                [    8,    0,             0,                           1,      0,       0], // init
                [    9,    0,             0,                           1,      0,       0], // init
                [    10,   0,             0,                           1,      0,       0], // init
                // This is one part of the instructions added in the setup fn `execute_code()`
                [    10,   0,             5,                           0,      0,       1],
                [    11,   0,             0,                           1,      0,       0], // init
            ],
        );

        // Finally, append the above trace with the extra init rows with unused
        // registers.
        let mut final_init_rows = prep_table(
            (12..32).map(|i|
                // addr value augmented_clk  diff_augmented_clk  is_init is_read is_write
                [     i,   0,             0,                           1,      0,       0]
            ).collect(),
        );
        trace.iter().enumerate().for_each(|(i, row)| {
            log::error!("{:?}", (i, row.addr, row.clk, row.augmented_clk_()));
        });
        expected_trace.append(&mut final_init_rows);

        // log::error!("trace: {:?}", &trace);
        // Check the final trace.
        (0..expected_trace.len()).for_each(|i| {
            assert_eq!(
                trace[i], expected_trace[i],
                "Final trace is wrong at row {i}"
            );
        });

        // Check the paddings. Important checks:
        // 1) Padded address = 31, since it's in the last row.
        // 2) is_used = is_init + is_read + is_write = 0, for CTL
        // with the `RegisterInitStark`.
        (expected_trace.len()..trace.len()).for_each(|i| {
            assert_eq!(
                trace[i],
                Register {
                    addr: F::from_canonical_u8(31),
                    ..Default::default()
                },
                "Padding is wrong at row {i}"
            );
        });

        assert!(trace.len().is_power_of_two());
    }
}
