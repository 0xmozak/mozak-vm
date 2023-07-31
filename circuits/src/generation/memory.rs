use itertools::{self, Itertools};
use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::columns::MemoryColumnsView;
use crate::memory::trace::{
    get_memory_inst_addr, get_memory_inst_clk, get_memory_inst_op, get_memory_load_inst_value,
    get_memory_store_inst_value,
};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<MemoryColumnsView<F>>) -> Vec<MemoryColumnsView<F>> {
    let padding = match trace.last() {
        Some(l) => MemoryColumnsView {
            // Some columns need special treatment..
            mem_padding: F::ONE,
            mem_diff_addr: F::ZERO,
            mem_diff_addr_inv: F::ZERO,
            mem_diff_clk: F::ZERO,
            // .. and all other columns just have their last value duplicated.
            ..*l
        },
        None => MemoryColumnsView::default(),
    };
    trace.resize(trace.len().next_power_of_two(), padding);

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
#[allow(clippy::missing_panics_doc)]
pub fn generate_memory_trace<F: RichField>(step_rows: &[Row]) -> Vec<MemoryColumnsView<F>> {
    let filtered_step_rows = filter_memory_trace(step_rows);

    let mut trace: Vec<MemoryColumnsView<F>> = vec![];
    for s in &filtered_step_rows {
        let mut row = MemoryColumnsView::default();
        row.mem_addr = get_memory_inst_addr(s);
        row.mem_clk = get_memory_inst_clk(s);
        row.mem_op = get_memory_inst_op(&s.state.current_instruction());

        row.mem_value = match s.state.current_instruction().op {
            Op::LB => get_memory_load_inst_value(s),
            Op::SB => get_memory_store_inst_value(s),
            #[tarpaulin::skip]
            _ => F::ZERO,
        };

        row.mem_diff_addr = row.mem_addr - trace.last().map_or(F::ZERO, |last| last.mem_addr);
        row.mem_diff_addr_inv = row.mem_diff_addr.try_inverse().unwrap_or_default();
        row.mem_diff_clk = match trace.last() {
            Some(last) if row.mem_diff_addr == F::ZERO => row.mem_clk - last.mem_clk,
            _ => F::ZERO,
        };
        trace.push(row);
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

    use crate::memory::columns::{self as mem_cols, MemoryColumnsView};
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::memory::trace::{OPCODE_LB, OPCODE_SB};
    use crate::test_utils::inv;

    fn prep_table<F: RichField>(
        table: Vec<[u64; mem_cols::NUM_MEM_COLS]>,
    ) -> Vec<MemoryColumnsView<F>> {
        table
            .into_iter()
            .map(|row| row.into_iter().map(F::from_canonical_u64).collect())
            .collect()
    }

    fn expected_trace<F: RichField>() -> Vec<MemoryColumnsView<F>> {
        let sb = OPCODE_SB as u64;
        let lb = OPCODE_LB as u64;
        let inv = inv::<F>;
        #[rustfmt::skip]
        prep_table(vec![
            // PADDING  ADDR  CLK   OP  VALUE  DIFF_ADDR  DIFF_ADDR_INV  DIFF_CLK
            [ 0,       100,  0,    sb,   5,    100,     inv(100),              0],
            [ 0,       100,  1,    lb,   5,      0,           0,               1],
            [ 0,       100,  4,    sb,  10,      0,           0,               3],
            [ 0,       100,  5,    lb,  10,      0,           0,               1],
            [ 0,       200,  2,    sb,  15,    100,     inv(100),              0],
            [ 0,       200,  3,    lb,  15,      0,           0,               1],
            [ 1,       200,  3,    lb,  15,      0,           0,               0],
            [ 1,       200,  3,    lb , 15,      0,           0,               0],
        ])
    }

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte (LB) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    fn generate_memory_trace() {
        let rows = memory_trace_test_case();

        let trace = super::generate_memory_trace::<GoldilocksField>(&rows);
        assert_eq!(trace, expected_trace());
    }

    #[test]
    fn generate_memory_trace_without_padding() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let rows = memory_trace_test_case();
        let trace = super::generate_memory_trace::<F>(&rows[..4]);

        let expected_trace: Vec<MemoryColumnsView<GoldilocksField>> = expected_trace();
        let expected_trace: Vec<MemoryColumnsView<GoldilocksField>> = vec![
            expected_trace[0],
            expected_trace[1],
            expected_trace[4],
            expected_trace[5],
        ];

        assert_eq!(trace, expected_trace);
    }
}
