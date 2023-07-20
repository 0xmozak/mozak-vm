use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::columns as mem_cols;
use crate::memory::trace::{
    get_memory_inst_addr, get_memory_inst_clk, get_memory_inst_op, get_memory_load_inst_value,
    get_memory_store_inst_value,
};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<Vec<F>>) -> Vec<Vec<F>> {
    let ext_trace_len = trace[0].len().next_power_of_two();

    // Some columns need special treatment..
    trace[mem_cols::MEM_PADDING].resize(ext_trace_len, F::ONE);
    trace[mem_cols::MEM_DIFF_ADDR].resize(ext_trace_len, F::ZERO);
    trace[mem_cols::MEM_DIFF_ADDR_INV].resize(ext_trace_len, F::ZERO);
    trace[mem_cols::MEM_DIFF_CLK].resize(ext_trace_len, F::ZERO);

    // .. and all other columns just have their last value duplicated.
    for row in &mut trace {
        row.resize(ext_trace_len, *row.last().unwrap());
    }

    trace
}

/// Returns the rows sorted in the order of the instruction address.
#[must_use]
pub fn filter_memory_trace(mut step_rows: Vec<Row>) -> Vec<Row> {
    step_rows.retain(|row| row.aux.mem_addr.is_some());

    // Sorting is stable, and rows are already ordered by row.state.clk
    step_rows.sort_by_key(|row| row.aux.mem_addr);

    step_rows
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_memory_trace<F: RichField>(
    step_rows: Vec<Row>,
) -> [Vec<F>; mem_cols::NUM_MEM_COLS] {
    let filtered_step_rows = filter_memory_trace(step_rows);
    let trace_len = filtered_step_rows.len();

    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; trace_len]; mem_cols::NUM_MEM_COLS];
    for (i, s) in filtered_step_rows.iter().enumerate() {
        trace[mem_cols::MEM_ADDR][i] = get_memory_inst_addr(s);
        trace[mem_cols::MEM_CLK][i] = get_memory_inst_clk(s);
        trace[mem_cols::MEM_OP][i] = get_memory_inst_op(&s.state.current_instruction());

        trace[mem_cols::MEM_VALUE][i] = match s.state.current_instruction().op {
            Op::LB => get_memory_load_inst_value(s),
            Op::SB => get_memory_store_inst_value(s),
            #[tarpaulin::skip]
            _ => F::ZERO,
        };

        trace[mem_cols::MEM_DIFF_ADDR][i] = trace[mem_cols::MEM_ADDR][i]
            - if i == 0 {
                F::ZERO
            } else {
                trace[mem_cols::MEM_ADDR][i - 1]
            };

        trace[mem_cols::MEM_DIFF_ADDR_INV][i] = trace[mem_cols::MEM_DIFF_ADDR][i]
            .try_inverse()
            .unwrap_or_default();

        trace[mem_cols::MEM_DIFF_CLK][i] = if i == 0 || trace[mem_cols::MEM_DIFF_ADDR][i] != F::ZERO
        {
            F::ZERO
        } else {
            trace[mem_cols::MEM_CLK][i] - trace[mem_cols::MEM_CLK][i - 1]
        };
    }

    // If the trace length is not a power of two, we need to extend the trace to the
    // next power of two. The additional elements are filled with the last row
    // of the trace.
    trace = pad_mem_trace(trace);

    #[tarpaulin::skip]
    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            mem_cols::NUM_MEM_COLS,
            v.len()
        )
    })
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use plonky2::hash::hash_types::RichField;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::memory::columns as mem_cols;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::memory::trace::{OPCODE_LB, OPCODE_SB};
    use crate::test_utils::inv;

    /// Transposes a table
    ///
    /// Like x[i][j] == transpose(x)[j][i]
    fn transpose<A: Clone>(table: &[&[A]]) -> Vec<Vec<A>> {
        // TODO(Matthias): find something in itertools or so to transpose, instead of
        // doing it manually. Otherwise, move this function to its own crate,
        // polish and publish it?
        // In any case, this is probably useful for some of the other tests as well.
        let mut table: Vec<(usize, &A)> = table
            .iter()
            .flat_map(|row| row.iter().enumerate())
            .collect();
        table.sort_by_key(|(col, _item)| *col);
        table
            .into_iter()
            .group_by(|(col, _item)| *col)
            .into_iter()
            .map(|(_, col)| col.map(|(_, item)| item).cloned().collect())
            .collect()
    }

    fn prep_table<F: RichField>(table: &[&[u64]]) -> [Vec<F>; mem_cols::NUM_MEM_COLS] {
        transpose(table)
            .into_iter()
            .map(|col| col.into_iter().map(F::from_canonical_u64).collect())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    fn expected_trace<F: RichField>() -> [Vec<F>; mem_cols::NUM_MEM_COLS] {
        let sb = OPCODE_SB as u64;
        let lb = OPCODE_LB as u64;
        let inv = inv::<F>;
        #[rustfmt::skip]
        prep_table(&[
            // PADDING  ADDR  CLK   OP  VALUE  DIFF_ADDR  DIFF_ADDR_INV  DIFF_CLK
            &[ 0,       100,  0,    sb,   5,    100,     inv(100),              0],
            &[ 0,       100,  1,    lb,   5,      0,           0,               1],
            &[ 0,       100,  4,    sb,  10,      0,           0,               3],
            &[ 0,       100,  5,    lb,  10,      0,           0,               1],
            &[ 0,       200,  2,    sb,  15,    100,     inv(100),              0],
            &[ 0,       200,  3,    lb,  15,      0,           0,               1],
            &[ 1,       200,  3,    lb,  15,      0,           0,               0],
            &[ 1,       200,  3,    lb , 15,      0,           0,               0],
        ])
    }

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte (LB) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    fn generate_memory_trace() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let rows = memory_trace_test_case();

        let trace = super::generate_memory_trace::<F>(rows);
        assert_eq!(trace, expected_trace());
    }

    #[test]
    fn generate_memory_trace_without_padding() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let rows = memory_trace_test_case();
        let trace = super::generate_memory_trace::<F>(rows[..4].to_vec());

        let indices = [0, 1, 4, 5];
        let expected_trace_vec: Vec<Vec<F>> = expected_trace()
            .iter()
            .map(|v| indices.iter().filter_map(|&i| v.get(i).copied()).collect())
            .collect();

        let expected_trace: [Vec<F>; mem_cols::NUM_MEM_COLS] =
            expected_trace_vec.try_into().expect("Mismatched lengths");

        assert_eq!(trace, expected_trace);
    }
}
