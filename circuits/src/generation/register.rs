use std::ops::Index;

use itertools::Itertools;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::generation::MIN_TRACE_LENGTH;
use crate::memory_io::columns::InputOutputMemory;
use crate::register::columns::{dummy, Ops, Register, RegisterCtl};
use crate::register_zero::columns::RegisterZero;
use crate::registerinit::columns::RegisterInit;
use crate::stark::mozak_stark::{Lookups, RegisterLookups, Table, TableKind};
use crate::utils::pad_trace_with_default;

/// Sort rows into blocks of ascending addresses, and then sort each block
/// internally by `augmented_clk`
#[must_use]
pub fn sort_into_address_blocks<F: RichField>(mut trace: Vec<Register<F>>) -> Vec<Register<F>> {
    trace.sort_by_key(|row| {
        (
            row.addr.to_noncanonical_u64(),
            row.augmented_clk().to_noncanonical_u64(),
        )
    });
    trace
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

// TODO: unify this with the `fn extract` in `generation/rangecheck.rs`.
pub fn extract_raw<'a, F: RichField, V>(trace: &[V], looking_table: &Table) -> Vec<Vec<F>>
where
    V: Index<usize, Output = F> + 'a, {
    trace
        .iter()
        .circular_tuple_windows()
        .filter(|&(prev_row, row)| looking_table.filter_column.eval(prev_row, row).is_one())
        .map(|(prev_row, row)| {
            looking_table
                .columns
                .iter()
                .map(|column| column.eval(prev_row, row))
                .collect_vec()
        })
        .collect()
}

// At the moment, we need cpu and memory traces.
pub fn extract<'a, F: RichField, V>(trace: &[V], looking_table: &Table) -> Vec<Register<F>>
where
    V: Index<usize, Output = F> + 'a, {
    let values: Vec<_> = extract_raw(trace, looking_table);
    values
        .into_iter()
        .map(|value| {
            let RegisterCtl {
                addr,
                value,
                clk,
                op,
            } = value.into_iter().collect();
            let ops = Ops::from(op);
            Register {
                addr,
                value,
                clk,
                ops,
            }
        })
        .collect()
}

#[must_use]
/// Generates the trace for registers.
///
/// There are 3 steps:
/// 1) populate the trace with a similar layout as the
/// [`RegisterInit` table](crate::registerinit::columns),
/// 2) go through the program and extract all ops that act on registers,
/// filling up this table,
/// 3) pad with dummy rows (`is_used` == 0) to ensure that trace is a power of
///    2.
pub fn generate_register_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    mem_private: &[InputOutputMemory<F>],
    mem_public: &[InputOutputMemory<F>],
    mem_transcript: &[InputOutputMemory<F>],
    reg_init: &[RegisterInit<F>],
) -> (Vec<RegisterZero<F>>, Vec<Register<F>>) {
    // TODO: handle multiplicities?
    let operations: Vec<Register<F>> = RegisterLookups::lookups()
        .looking_tables
        .into_iter()
        .flat_map(|looking_table| match looking_table.kind {
            TableKind::Cpu => extract(cpu_trace, &looking_table),
            TableKind::IoMemoryPrivate => extract(mem_private, &looking_table),
            TableKind::IoMemoryPublic => extract(mem_public, &looking_table),
            TableKind::IoTranscript => extract(mem_transcript, &looking_table),
            TableKind::RegisterInit => extract(reg_init, &looking_table),
            // Flow of information in generation goes in the other direction.
            TableKind::RegisterZero => vec![],
            other => unimplemented!("Can't extract register ops from {other:#?} tables"),
        })
        .collect();
    let trace = sort_into_address_blocks(operations);
    let (zeros, rest): (Vec<_>, Vec<_>) = trace.into_iter().partition(|row| row.addr.is_zero());
    log::trace!("trace {:?}", rest);

    let zeros = zeros.into_iter().map(RegisterZero::from).collect();

    (pad_trace_with_default(zeros), pad_trace(rest))
}

#[cfg(test)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::util::execute_code;
    use mozak_runner::vm::ExecutionRecord;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
        generate_io_transcript_trace,
    };
    use crate::generation::registerinit::generate_register_init_trace;
    use crate::test_utils::prep_table;

    type F = GoldilocksField;

    fn setup() -> ExecutionRecord<F> {
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

        execute_code(instructions, &[], &[(6, 100), (7, 200)]).1
    }

    #[test]
    fn generate_reg_trace() {
        let record = setup();

        let cpu_rows = generate_cpu_trace::<F>(&record);
        let io_memory_private = generate_io_memory_private_trace(&record.executed);
        let io_memory_public = generate_io_memory_public_trace(&record.executed);
        let io_transcript = generate_io_transcript_trace(&record.executed);
        let register_init = generate_register_init_trace(&record);
        let (_zero, trace) = generate_register_trace(
            &cpu_rows,
            &io_memory_private,
            &io_memory_public,
            &io_transcript,
            &register_init,
        );

        // This is the actual trace of the instructions.
        #[rustfmt::skip]
        let mut expected_trace: Vec<Register<GoldilocksField>> = prep_table(
            vec![
                // First, populate the table with the instructions from the above test code.
                // Note that we filter out operations that act on r0.
                //
                // Columns:
                // addr value clk  is_init is_read is_write
                [    1,    0,   0,       1,      0,       0], // init
                [    2,    0,   0,       1,      0,       0], // init
                [    3,    0,   0,       1,      0,       0], // init
                [    4,    0,   0,       1,      0,       0], // init
                [    4,  300,   2,       0,      0,       1], // 1st inst
                [    4,  300,   3,       0,      1,       0], // 2nd inst
                [    4,  500,   4,       0,      0,       1], // 3rd inst
                [    5,    0,   0,       1,      0,       0], // init
                [    5,  400,   3,       0,      0,       1], // 2nd inst
                [    5,  400,   4,       0,      1,       0], // 3rd inst
                [    6,  100,   0,       1,      0,       0], // init
                [    6,  100,   2,       0,      1,       0], // 1st inst
                [    6,  100,   3,       0,      1,       0], // 2nd inst
                [    7,  200,   0,       1,      0,       0], // init
                [    7,  200,   2,       0,      1,       0], // 1st inst
                [    8,    0,   0,       1,      0,       0], // init
                [    9,    0,   0,       1,      0,       0], // init
                [    10,   0,   0,       1,      0,       0], // init
                // This is one part of the instructions added in the setup fn `execute_code()`
                [    10,   0,   5,       0,      0,       1],
                [    10,   0,   6,       0,      1,       0],
                [    11,   0,   0,       1,      0,       0], // init
                [    11,   0,   6,       0,      1,       0],
                [    12,   0,   0,       1,      0,       0], // init
            ],
        );

        // Finally, append the above trace with the extra init rows with unused
        // registers.
        #[rustfmt::skip]
        let mut final_init_rows = prep_table(
            (13..32).map(|i|
                // addr value clk  is_init is_read is_write
                [     i,   0,   0,       1,      0,       0]
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
