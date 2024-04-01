use itertools::{chain, izip, Itertools};
use mozak_runner::instruction::Args;
use mozak_runner::state::State;
use mozak_runner::vm::ExecutionRecord;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::register::columns::{init, read, write, Ops, Register};

/// Sort rows into blocks of ascending addresses, and then sort each block
/// internally by `augmented_clk`
#[must_use]
pub fn sort_into_address_blocks<F: RichField>(mut trace: Vec<Register<F>>) -> Vec<Register<F>> {
    trace.sort_by_key(|row| {
        (
            row.addr.to_noncanonical_u64(),
            row.augmented_clk.to_noncanonical_u64(),
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
        ops: Ops::default(),
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

    let build_single_register_trace_row =
        |reg: fn(&Args) -> u8, ops: Ops<F>, clk_offset: u64| -> _ {
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
                        augmented_clk: F::from_canonical_u64(row.state.clk * 3 + clk_offset),
                        ops,
                        ..Default::default()
                    }
                })
        };
    let trace = sort_into_address_blocks(
        chain!(
            init_register_trace(record.executed.first().map_or(last_state, |row| &row.state)),
            build_single_register_trace_row(|Args { rs1, .. }| *rs1, read(), 0),
            build_single_register_trace_row(|Args { rs2, .. }| *rs2, read(), 1),
            build_single_register_trace_row(|Args { rd, .. }| *rd, write(), 2)
        )
        .collect_vec(),
    );

    // Populate the `diff_augmented_clk` column, after addresses are sorted.
    // TODO: Consider rewriting this to avoid allocating a temp vector.
    let mut diff_augmented_clk = trace
        .iter()
        .circular_tuple_windows()
        .map(|(lv, nv)| nv.augmented_clk - lv.augmented_clk)
        .collect_vec();
    // `.circular_tuple_windows` gives us tuples with indices (0, 1), (1, 2) ..
    // (last, first==0), but we need (last, first=0), (0, 1), .. (last-1, last).
    diff_augmented_clk.rotate_right(1);

    pad_trace(
        izip!(trace, diff_augmented_clk)
            .map(|(reg, diff_augmented_clk)| Register {
                diff_augmented_clk,
                ..reg
            })
            .collect_vec(),
    )
}

#[cfg(test)]
mod tests {
    use mozak_runner::elf::Program;
    use mozak_runner::instruction::{Instruction, Op};
    use mozak_runner::util::execute_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, PrimeField64};

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
                    [     i, value,            0,                 0,        1,      0,      0]
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

    fn neg(val: u64) -> u64 { (F::ZERO - F::from_canonical_u64(val)).to_canonical_u64() }

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
                // addr value augmented_clk  diff_augmented_clk  is_init is_read is_write
                [    1,    0,             0,                 0,        1,      0,       0], // init
                [    2,    0,             0,                 0,        1,      0,       0], // init
                [    3,    0,             0,                 0,        1,      0,       0], // init
                [    4,    0,             0,                 0,        1,      0,       0], // init
                [    4,  300,             8,                 8,        0,      0,       1], // 1st inst
                [    4,  300,             9,                 1,        0,      1,       0], // 2nd inst
                [    4,  500,            14,                 5,        0,      0,       1], // 3rd inst
                [    5,    0,             0,           neg(14),        1,      0,       0], // init
                [    5,  400,            11,                11,        0,      0,       1], // 2nd inst
                [    5,  400,            12,                 1,        0,      1,       0], // 3rd inst
                [    6,  100,             0,           neg(12),        1,      0,       0], // init
                [    6,  100,             6,                 6,        0,      1,       0], // 1st inst
                [    6,  100,            10,                 4,        0,      1,       0], // 2nd inst
                [    7,  200,             0,           neg(10),        1,      0,       0], // init
                [    7,  200,             7,                 7,        0,      1,       0], // 1st inst
                [    8,    0,             0,            neg(7),        1,      0,       0], // init
                [    9,    0,             0,                 0,        1,      0,       0], // init
                [    10,   0,             0,                 0,        1,      0,       0], // init
                // This is one part of the instructions added in the setup fn `execute_code()`
                [    10,   0,            17,                17,        0,      0,       1],
                [    10,   0,            18,                 1,        0,      1,       0],
                [    11,   0,             0,           neg(18),        1,      0,       0], // init
                [    11,   0,            19,                19,        0,      1,       0],
                [    12,   0,             0,           neg(19),        1,      0,       0], // init
            ],
        );

        // Finally, append the above trace with the extra init rows with unused
        // registers.
        let mut final_init_rows = prep_table(
            (13..32).map(|i|
                // addr value augmented_clk  diff_augmented_clk  is_init is_read is_write
                [     i,   0,             0,                 0,        1,      0,       0]
            ).collect(),
        );
        expected_trace.append(&mut final_init_rows);

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
