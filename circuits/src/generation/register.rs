use itertools::Itertools;
use mozak_runner::elf::Program;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use crate::register::columns::Register;

/// Returns the rows sorted in the order of the register 'address'.
#[must_use]
pub fn sort_by_addr<F: RichField>(trace: Vec<Register<F>>) -> Vec<Register<F>> {
    trace
        .into_iter()
        // Sorting is stable, and rows are already ordered by row.state.clk
        .sorted_by_key(|row| row.addr.to_noncanonical_u64())
        .collect()
}

fn init_register_trace<F: RichField>() -> Vec<Register<F>> {
    (1..32)
        .map(|i| Register {
            addr: F::from_canonical_usize(i),
            is_init: F::ONE,
            ..Default::default()
        })
        .collect()
}

/// Generates the trace for registers.
///
/// There are 3 steps:
/// 1) populate the trace with a similar layout as the
/// [`RegisterInit` table](crate::registerinit::columns),
/// 2) go through the program and extract all ops that act on registers,
/// 3) pad with dummy rows.
#[must_use]
pub fn generate_register_trace<F: RichField>(
    program: &Program,
    record: &ExecutionRecord,
) -> Vec<Register<F>> {
    let ExecutionRecord { executed, .. } = record;

    let mut trace = init_register_trace();

    for Row { state, .. } in executed {
        let inst = state.current_instruction(program);

        (inst.args.rs1 != 0).then(|| {
            trace.append(&mut vec![Register {
                addr: F::from_canonical_u8(inst.args.rs1),
                did_addr_change: F::ZERO,
                value: F::from_canonical_u32(state.get_register_value(inst.args.rs1)),
                augmented_clk: F::from_canonical_u64((state.clk) * 2),
                is_init: F::ZERO,
                is_read: F::ONE,
                is_write: F::ZERO,
            }]);
        });

        (inst.args.rs2 != 0).then(|| {
            trace.append(&mut vec![Register {
                addr: F::from_canonical_u8(inst.args.rs2),
                did_addr_change: F::ZERO,
                value: F::from_canonical_u32(state.get_register_value(inst.args.rs2)),
                augmented_clk: F::from_canonical_u64((state.clk) * 2),
                is_init: F::ZERO,
                is_read: F::ONE,
                is_write: F::ZERO,
            }]);
        });

        (inst.args.rd != 0).then(|| {
            trace.append(&mut vec![Register {
                addr: F::from_canonical_u8(inst.args.rd),
                did_addr_change: F::ZERO,
                value: F::from_canonical_u32(state.get_register_value(inst.args.rd)),
                augmented_clk: F::from_canonical_u64((state.clk) * 2 + 1),
                is_init: F::ZERO,
                is_read: F::ZERO,
                is_write: F::ONE,
            }]);
        });
    }

    sort_by_addr(trace)
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use log::debug;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;

    use super::*;
    use crate::columns_view::NumberOfColumns;
    use crate::test_utils::prep_table;

    type F = GoldilocksField;

    #[test]
    fn generate_reg_trace_initial() {
        let trace = init_register_trace();
        let expected_trace = prep_table::<F, Register<F>, { Register::<F>::NUMBER_OF_COLUMNS }>(
            (1..32)
                .map(|i|
                // Columns (repeated for registers 0-31):
                // addr did_addr_change value augmented_clk is_init is_read is_write
                [         i,              0,    0,            0,      1,      0,       0])
                .collect_vec(),
        );
        (0..31).for_each(|i| {
            assert_eq!(
                trace[i], expected_trace[i],
                "Initial trace setup is wrong at row {i}"
            );
        });
    }

    #[test]
    fn generate_reg_trace() {
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

        let (program, record) = simple_test_code(&instructions, &[], &[(6, 100), (7, 200)]);

        // TODO: generate this from cpu rows?
        // For now, use program and record directly to avoid changing the CPU columns
        // yet.
        let trace = generate_register_trace::<F>(&program, &record);

        // This is just the initial trace, similar to structure of
        // [`RegisterInit`](registerinit).
        let mut expected_trace = prep_table::<F, Register<F>, { Register::<F>::NUMBER_OF_COLUMNS }>(
            (1..32)
                .map(|i|
                // Columns (repeated for registers 1-31):
                // addr did_addr_change value augmented_clk is_init is_read is_write
                [         i,              0,    0,            0,      1,      0,       0])
                .collect_vec(),
        );

        // This is the unsorted trace consisting of the init table, entries from the
        // instructions and the cleanup instructions from `simple_test_code()`.
        expected_trace.extend_from_slice(
            #[rustfmt::skip]
            &prep_table::<F, Register<F>, { Register::<F>::NUMBER_OF_COLUMNS }>(vec![
                // First, populate the table with the instructions from the above test code.
                //
                // Columns:
                // addr did_addr_change value augmented_clk is_init is_read is_write
                //
                // Instructions: in order of (rs1, rs2/imm, rd)
                // ADD r6, r7, r4
                [        6,              0,  100,            2,      0,      1,       0],
                [        7,              0,  200,            2,      0,      1,       0],
                [        4,              0,  0,              3,      0,      0,       1],
                // ADD r4, r6, r5
                [        4,              0,  300,            4,      0,      1,       0],
                [        6,              0,  100,            4,      0,      1,       0],
                [        5,              0,  0,              5,      0,      0,       1],
                // ADD r5, 100, r4 (note: imm values are ignored)
                [        5,              0,  400,            6,      0,      1,       0],
                [        4,              0,  300,            7,      0,      0,       1],
                // Next, we add the instructions added in `simple_test_code()`
                // Note that we filter out operations that act on r0.
                [        10,             0,  0,              9,      0,      0,       1],
            ]),
        );

        // Finally, this is the sorted trace, where we populate `did_addr_change`.
        let expected_trace = sort_by_addr(expected_trace);

        debug!("{:#?}", trace);
        (0..trace.len()).for_each(|i| {
            assert_eq!(
                trace[i], expected_trace[i],
                "Final trace is wrong at row {i}"
            );
        });
    }
}
