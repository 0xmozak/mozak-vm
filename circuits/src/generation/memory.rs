use itertools::{self, chain};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::columns::Memory;
use crate::memory::trace::{get_memory_inst_addr, get_memory_inst_clk};
use crate::memory_fullword::columns::FullWordMemory;
use crate::memory_halfword::columns::HalfWordMemory;
use crate::memoryinit::columns::MemoryInit;

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<Memory<F>>) -> Vec<Memory<F>> {
    trace.resize(trace.len().next_power_of_two().max(4), Memory {
        // Some columns need special treatment..
        is_store: F::ZERO,
        is_load: F::ZERO,
        is_init: F::ZERO,
        diff_addr: F::ZERO,
        diff_addr_inv: F::ZERO,
        diff_clk: F::ZERO,
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Generates Memory trace from dynamic VM execution of
/// `Program`. These need to be further interleaved with
/// static memory trace generated from `Program` for final
/// execution for final memory trace.
pub fn generate_memory_trace_from_execution<'a, F: RichField>(
    program: &'a Program,
    step_rows: &'a [Row],
) -> impl Iterator<Item = Memory<F>> + 'a {
    step_rows
        .iter()
        .filter(|row| {
            row.aux.mem.is_some()
                && matches!(
                    row.state.current_instruction(program).op,
                    Op::LB | Op::LBU | Op::SB
                )
        })
        .map(|row| {
            let addr: F = get_memory_inst_addr(row);
            let op = &(row.state).current_instruction(program).op;
            Memory {
                addr,
                clk: get_memory_inst_clk(row),
                is_store: F::from_bool(matches!(op, Op::SB)),
                is_load: F::from_bool(matches!(op, Op::LB | Op::LBU)),
                is_init: F::ZERO,
                value: F::from_canonical_u32(row.aux.dst_val),
                ..Default::default()
            }
        })
}

/// Generates Memory trace from a memory init table.
///
/// These need to be further interleaved with runtime memory trace generated
/// from VM execution for final memory trace.
pub fn transform_memory_init<F: RichField>(
    memory_init_rows: &[MemoryInit<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    memory_init_rows
        .iter()
        .filter_map(Option::<Memory<F>>::from)
}

/// Generates Memory trace from a memory half-word table.
///
/// These need to be further interleaved with runtime memory trace generated
/// from VM execution for final memory trace.
pub fn transform_halfword<F: RichField>(
    halfword_memory: &[HalfWordMemory<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    halfword_memory
        .iter()
        .flat_map(Into::<Vec<Memory<F>>>::into)
}

/// Generates Memory trace from a memory full-word table.
///
/// These need to be further interleaved with runtime memory trace generated
/// from VM execution for final memory trace.
pub fn transform_fullword<F: RichField>(
    fullword_memory: &[FullWordMemory<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    fullword_memory
        .iter()
        .flat_map(Into::<Vec<Memory<F>>>::into)
}

fn key<F: RichField>(memory: &Memory<F>) -> (u64, u64) {
    (
        memory.addr.to_canonical_u64(),
        memory.clk.to_canonical_u64(),
    )
}

/// Generates memory trace using static component `program` for
/// memory initialization and dynamic component `step_rows` for
/// access (load and store) of memory elements. Trace constraints
/// are supposed to abide by read-only and read-write address
/// constraints.
#[must_use]
pub fn generate_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row],
    memory_init_rows: &[MemoryInit<F>],
    halfword_memory_rows: &[HalfWordMemory<F>],
    fullword_memory_rows: &[FullWordMemory<F>],
) -> Vec<Memory<F>> {
    // `merged_trace` is address sorted combination of static and
    // dynamic memory trace components of program (ELF and execution)
    // `merge` operation is expected to be stable
    let mut merged_trace: Vec<Memory<F>> = chain!(
        transform_memory_init::<F>(memory_init_rows),
        generate_memory_trace_from_execution(program, step_rows),
        transform_halfword(halfword_memory_rows),
        transform_fullword(fullword_memory_rows),
    )
    .collect();
    merged_trace.sort_by_key(key);

    // Ensures constraints by filling remaining inter-row
    // relation values: clock difference and addr difference and is_writable
    let mut last_clk = F::ZERO;
    let mut last_addr = F::ZERO;
    let mut last_is_writable = F::ZERO;
    for mem in &mut merged_trace {
        mem.diff_addr = mem.addr - last_addr;
        mem.diff_addr_inv = mem.diff_addr.try_inverse().unwrap_or_default();
        if mem.addr == last_addr {
            mem.diff_clk = mem.clk - last_clk;
        }
        (last_clk, last_addr) = (mem.clk, mem.addr);
        // rows with is_init set are the source of truth about is_writable
        if mem.is_init.is_one() {
            last_is_writable = mem.is_writable;
        }
        mem.is_writable = last_is_writable;
    }

    // If the trace length is not a power of two, we need to extend the trace to the
    // next power of two. The additional elements are filled with the last row
    // of the trace.
    pad_mem_trace(merged_trace)
}

#[cfg(test)]
mod tests {
    use im::hashmap::HashMap;
    use mozak_runner::elf::{Data, Program};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::test_utils::{inv, prep_table};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte unsigned (LBU) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    #[rustfmt::skip]
    fn generate_memory_trace() {
        let (program, record) = memory_trace_test_case(1);

        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&program, &record.executed);
        let fullword_memory = generate_fullword_memory_trace(&program, &record.executed);

        let trace = super::generate_memory_trace::<GoldilocksField>(
            &program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
        );
        let inv = inv::<F>;
        assert_eq!(
            trace,
            prep_table(vec![
                //is_writable  addr   clk  is_sb, is_lbu, is_init  value  diff_addr  diff_addr_inv  diff_clk
                [       1,     100,   0,     0,     0,       1,        0,    100,     inv(100),            0],  // Memory Init: 100
                [       1,     100,   1,     1,     0,       0,      255,      0,           0,             1],  // Operations:  100
                [       1,     100,   2,     0,     1,       0,      255,      0,           0,             1],  // Operations:  100
                [       1,     100,   5,     1,     0,       0,       10,      0,           0,             3],  // Operations:  100
                [       1,     100,   6,     0,     1,       0,       10,      0,           0,             1],  // Operations:  100
                [       1,     101,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 101
                [       1,     102,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 102
                [       1,     103,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 103
                [       1,     200,   0,     0,     0,       1,        0,     97,     inv(97),             0],  // Memory Init: 200
                [       1,     200,   3,     1,     0,       0,       15,      0,           0,             3],  // Operations:  200
                [       1,     200,   4,     0,     1,       0,       15,      0,           0,             1],  // Operations:  200
                [       1,     201,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 201
                [       1,     202,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 202
                [       1,     203,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 203
                [       1,     203,   0,     0,     0,       0,        0,      0,           0,             0],  // Padding
                [       1,     203,   0,     0,     0,       0,        0,      0,           0,             0],  // Padding
            ])
        );
    }

    #[test]
    #[rustfmt::skip]
    fn generate_memory_trace_only_init() {
        let program = Program {
            ro_memory: Data(
                [(100, 5), (101, 6)]
                    .iter()
                    .copied()
                    .collect::<HashMap<u32, u8>>(),
            ),
            rw_memory: Data(
                [(200, 7), (201, 8)]
                    .iter()
                    .copied()
                    .collect::<HashMap<u32, u8>>(),
            ),
            ..Program::default()
        };

        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&program, &[]);
        let fullword_memory = generate_fullword_memory_trace(&program, &[]);
        let trace = super::generate_memory_trace::<F>(
            &program,
            &[],
            &memory_init,
            &halfword_memory,
            &fullword_memory,
        );

        let inv = inv::<F>;
        assert_eq!(trace, prep_table(vec![
            // is_writable   addr   clk  is_sb, is_lbu, is_init   value  diff_addr  diff_addr_inv  diff_clk
            [        0,      100,   0,      0,    0,    1,       5,    100,    inv(100),             0],
            [        0,      101,   0,      0,    0,    1,       6,      1,           1,             0],
            [        1,      200,   0,      0,    0,    1,       7,     99,     inv(99),             0],
            [        1,      201,   0,      0,    0,    1,       8,      1,           1,             0],
        ]));
    }
}
