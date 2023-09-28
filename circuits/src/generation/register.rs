use itertools::Itertools;
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use crate::register::columns::Register;

/// Returns the rows sorted in the order of the register 'address'.
#[must_use]
pub fn sort_by_address<F: RichField>(trace: Vec<Register<F>>) -> Vec<Register<F>> {
    trace
        .into_iter()
        // Sorting is stable, and rows are already ordered by row.state.clk
        .sorted_by_key(|row| row.addr.to_noncanonical_u64())
        .collect()
}

fn init_register_trace<F: RichField>(state: &State) -> Vec<Register<F>> {
    (1..32)
        .map(|i| Register {
            addr: F::from_canonical_u8(i),
            is_init: F::ONE,
            value: F::from_canonical_u32(state.get_register_value(i)),
            ..Default::default()
        })
        .collect()
}

#[must_use]
pub fn pad_trace<F: RichField>(mut trace: Vec<Register<F>>) -> Vec<Register<F>> {
    let len = trace.len().next_power_of_two();
    trace.resize(len, Register {
        // We want these 3 filter columns = 0,
        // so we can constrain is_dummy = is_init + is_read + is_write.
        is_init: F::ZERO,
        is_read: F::ZERO,
        is_write: F::ZERO,
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
/// 3) pad with dummy rows to ensure that trace is a power of 2.
#[must_use]
pub fn generate_register_trace<F: RichField>(
    program: &Program,
    record: &ExecutionRecord,
) -> Vec<Register<F>> {
    let ExecutionRecord {
        executed,
        last_state,
    } = record;

    let mut trace =
        init_register_trace(record.executed.first().map_or(last_state, |row| &row.state));

    for Row { state, .. } in executed {
        let inst = state.current_instruction(program);

        let augmented_clk = F::from_canonical_u64((state.clk) * 3);

        // Ignore r0 because r0 should always be 0.
        // TODO: assert r0 = 0 constraint in CPU trace.
        (inst.args.rs1 != 0).then(|| {
            trace.append(&mut vec![Register {
                addr: F::from_canonical_u8(inst.args.rs1),
                value: F::from_canonical_u32(state.get_register_value(inst.args.rs1)),
                augmented_clk,
                diff_augmented_clk: augmented_clk - trace.last().unwrap().augmented_clk,
                is_init: F::ZERO,
                is_read: F::ONE,
                is_write: F::ZERO,
            }]);
        });

        (inst.args.rs2 != 0).then(|| {
            trace.append(&mut vec![Register {
                addr: F::from_canonical_u8(inst.args.rs2),
                value: F::from_canonical_u32(state.get_register_value(inst.args.rs2)),
                augmented_clk: augmented_clk + F::ONE,
                diff_augmented_clk: augmented_clk + F::ONE - trace.last().unwrap().augmented_clk,
                is_init: F::ZERO,
                is_read: F::ONE,
                is_write: F::ZERO,
            }]);
        });

        (inst.args.rd != 0).then(|| {
            trace.append(&mut vec![Register {
                addr: F::from_canonical_u8(inst.args.rd),
                value: F::from_canonical_u32(state.get_register_value(inst.args.rd)),
                augmented_clk: augmented_clk + F::TWO,
                diff_augmented_clk: augmented_clk + F::TWO - trace.last().unwrap().augmented_clk,
                is_init: F::ZERO,
                is_read: F::ZERO,
                is_write: F::ONE,
            }]);
        });
    }

    pad_trace(sort_by_address(trace))
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::columns_view::NumberOfColumns;
    use crate::test_utils::prep_table;

    type F = GoldilocksField;

    fn setup() -> (Program, ExecutionRecord) {
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

        simple_test_code(&instructions, &[], &[(6, 100), (7, 200)])
    }

    fn expected_trace<F: RichField>() -> Vec<Register<F>>
    where
        [(); Register::<F>::NUMBER_OF_COLUMNS]:, {
        #[rustfmt::skip]
        prep_table::<F, Register<F>, { Register::<F>::NUMBER_OF_COLUMNS }>(
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
        let expected_trace = expected_trace();
        (0..31).for_each(|i| {
            assert_eq!(
                trace[i], expected_trace[i],
                "Initial trace setup is wrong at row {i}"
            );
        });
    }

    #[test]
    fn generate_reg_trace() {
        let (program, record) = setup();

        // TODO: generate this from cpu rows?
        // For now, use program and record directly to avoid changing the CPU columns
        // yet.
        let trace = generate_register_trace::<F>(&program, &record);

        // This is just the initial trace, similar to structure of
        // [`RegisterInit`](registerinit).
        let mut expected_trace = expected_trace();

        // This is the unsorted trace consisting of the init table, entries from the
        // instructions and the cleanup instructions from `simple_test_code()`.
        expected_trace.extend_from_slice(
            #[rustfmt::skip]
            &prep_table::<F, Register<F>, { Register::<F>::NUMBER_OF_COLUMNS }>(vec![
                // First, populate the table with the instructions from the above test code.
                //
                // Columns:
                // addr value augmented_clk  diff_augmented_clk  is_init is_read is_write
                [    6,  100,             3,                 3,        0,      1,       0], // ADD r6,
                [    7,  200,             4,                 1,        0,      1,       0], //     r7,
                [    4,    0,             5,                 1,        0,      0,       1], //     r4
                [    4,  300,             6,                 1,        0,      1,       0], // ADD r4,
                [    6,  100,             7,                 1,        0,      1,       0], //     r6,
                [    5,    0,             8,                 1,        0,      0,       1], //     r5
                [    5,  400,             9,                 1,        0,      1,       0], // ADD r5 100
                [    4,  300,             11,                2,        0,      0,       1], //     r4
                // Next, we add the instructions added in `simple_test_code()`
                // Note that we filter out operations that act on r0.
                [    10,   0,             14,                3,        0,      0,       1],
            ]),
        );

        // Finally, this is the sorted trace.
        let expected_trace = sort_by_address(expected_trace);

        (0..expected_trace.len()).for_each(|i| {
            println!("{:?}", trace[i]);
            assert_eq!(
                trace[i], expected_trace[i],
                "Final trace is wrong at row {i}"
            );
        });

        // Check the paddings. Important checks:
        // 1) Padded address = 31, since it's in the last row.
        // 2) is_dummy = is_init + is_read + is_write = 0, for CTL
        // with the `RegisterInitStark`.
        (expected_trace.len()..trace.len()).for_each(|i| {
            assert_eq!(
                trace[i],
                Register {
                    addr: F::from_canonical_u8(31),
                    ..Default::default()
                },
                "Final trace is wrong at row {i}"
            );
        });

        assert!(trace.len().is_power_of_two());
    }
}
