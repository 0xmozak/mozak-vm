use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::columns::Memory;
use crate::memory::trace::{get_memory_inst_addr, get_memory_inst_clk};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<Memory<F>>) -> Vec<Memory<F>> {
    trace.resize(trace.len().next_power_of_two(), Memory {
        // Some columns need special treatment..
        is_sb: F::ZERO,
        is_lbu: F::ZERO,
        is_init: F::ZERO,
        diff_addr: F::ZERO,
        diff_addr_inv: F::ZERO,
        diff_clk: F::ZERO,
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Returns the rows sorted in the order of the instruction address.
#[must_use]
pub fn filter_memory_trace(step_rows: &[Row]) -> Vec<&Row> {
    step_rows
        .iter()
        .filter(|row| row.aux.mem_addr.is_some())
        // Sorting is stable, and rows are already ordered by row.state.clk
        .sorted_by_key(|row| row.aux.mem_addr)
        .collect_vec()
}

#[must_use]
pub fn generate_memory_trace<F: RichField>(program: &Program, step_rows: &[Row]) -> Vec<Memory<F>> {
    let filtered_step_rows = filter_memory_trace(step_rows);

    let mut trace: Vec<Memory<F>> = vec![];
    for s in &filtered_step_rows {
        let inst = s.state.current_instruction(program);
        let mem_clk = get_memory_inst_clk(s);
        let mem_addr = get_memory_inst_addr(s);
        let mem_diff_addr = mem_addr - trace.last().map_or(F::ZERO, |last| last.addr);
        trace.push(Memory {
            is_writable: F::ONE,
            addr: mem_addr,
            clk: mem_clk,
            is_sb: F::from_bool(matches!(inst.op, Op::SB)),
            is_lbu: F::from_bool(matches!(inst.op, Op::LBU)),
            is_init: F::ZERO, // TODO(Supragya): To be populated when meminit entries are added
            value: F::from_canonical_u32(s.aux.dst_val),
            diff_addr: mem_diff_addr,
            diff_addr_inv: mem_diff_addr.try_inverse().unwrap_or_default(),
            diff_clk: match trace.last() {
                Some(last) if mem_diff_addr == F::ZERO => mem_clk - last.clk,
                _ => F::ZERO,
            },
        });
    }

    // If the trace length is not a power of two, we need to extend the trace to the
    // next power of two. The additional elements are filled with the last row
    // of the trace.
    pad_mem_trace(trace)
}

#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::hash::hash_types::RichField;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::memory::columns::{self as mem_cols, Memory};
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::test_utils::inv;

    fn prep_table<F: RichField>(table: Vec<[u64; mem_cols::NUM_MEM_COLS]>) -> Vec<Memory<F>> {
        table
            .into_iter()
            .map(|row| row.into_iter().map(F::from_canonical_u64).collect())
            .collect()
    }

    fn expected_trace<F: RichField>() -> Vec<Memory<F>> {
        let inv = inv::<F>;
        #[rustfmt::skip]
        prep_table(vec![
            // is_writable   addr  clk   is_sb, is_lbu, is_init, value  diff_addr  diff_addr_inv  diff_clk
            [        1,      100,  1,        1,      0,       0,  255,    100,     inv(100),            0],
            [        1,      100,  2,        0,      1,       0,  255,      0,           0,             1],
            [        1,      100,  5,        1,      0,       0,   10,      0,           0,             3],
            [        1,      100,  6,        0,      1,       0,   10,      0,           0,             1],
            [        1,      200,  3,        1,      0,       0,   15,    100,     inv(100),            0],
            [        1,      200,  4,        0,      1,       0,   15,      0,           0,             1],
            [        1,      200,  4,        0,      0,       0,   15,      0,           0,             0],
            [        1,      200,  4,        0,      0,       0,   15,      0,           0,             0],
        ])
    }

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte unsigned (LBU) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    fn generate_memory_trace() {
        let (program, record) = memory_trace_test_case(1);

        let trace = super::generate_memory_trace::<GoldilocksField>(&program, &record.executed);
        assert_eq!(trace, expected_trace());
    }

    #[test]
    fn generate_memory_trace_without_padding() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let (program, record) = memory_trace_test_case(1);
        let trace = super::generate_memory_trace::<F>(&program, &record.executed[..4]);

        let expected_trace: Vec<Memory<GoldilocksField>> = expected_trace();
        let expected_trace: Vec<Memory<GoldilocksField>> = vec![
            expected_trace[0],
            expected_trace[1],
            expected_trace[4],
            expected_trace[5],
        ];

        assert_eq!(trace, expected_trace);
    }
}
