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
    let trace_len = trace[0].len();
    let ext_trace_len = trace_len.next_power_of_two();

    for row in trace[mem_cols::COL_MEM_ADDR..mem_cols::COL_MEM_DIFF_CLK].iter_mut() {
        row.resize(ext_trace_len, *row.last().unwrap());
    }

    trace[mem_cols::COL_MEM_PADDING].resize(ext_trace_len, F::ONE);
    trace[mem_cols::COL_MEM_DIFF_ADDR].resize(ext_trace_len, F::ZERO);
    trace[mem_cols::COL_MEM_DIFF_ADDR_INV].resize(ext_trace_len, F::ZERO);
    trace[mem_cols::COL_MEM_DIFF_CLK].resize(ext_trace_len, F::ZERO);

    trace
}

/// Returns the rows sorted in the order of the instruction address.
#[must_use]
pub fn filter_memory_trace(mut step_rows: Vec<Row>) -> Vec<Row> {
    // Sorting is stable, and rows are already ordered by row.state.clk
    step_rows.sort_by_key(|row| {
        let data = row.state.current_instruction().data;
        row.state
            .get_register_value(data.rs1.into())
            .wrapping_add(data.imm)
    });

    step_rows
        .into_iter()
        .filter(|row| {
            let inst = row.state.current_instruction();
            inst.op == Op::LB || inst.op == Op::SB
        })
        .collect()
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
        trace[mem_cols::COL_MEM_ADDR][i] = get_memory_inst_addr(s);
        trace[mem_cols::COL_MEM_CLK][i] = get_memory_inst_clk(s);
        trace[mem_cols::COL_MEM_OP][i] = get_memory_inst_op(&s.state.current_instruction());

        trace[mem_cols::COL_MEM_VALUE][i] = match s.state.current_instruction().op {
            Op::LB => get_memory_load_inst_value(s),
            Op::SB => get_memory_store_inst_value(s),
            _ => F::ZERO,
        };

        trace[mem_cols::COL_MEM_DIFF_ADDR][i] = if i == 0 {
            F::ZERO
        } else {
            trace[mem_cols::COL_MEM_ADDR][i] - trace[mem_cols::COL_MEM_ADDR][i - 1]
        };

        trace[mem_cols::COL_MEM_DIFF_ADDR_INV][i] =
            if trace[mem_cols::COL_MEM_DIFF_ADDR][i] == F::ZERO {
                F::ZERO
            } else {
                trace[mem_cols::COL_MEM_DIFF_ADDR][i].inverse()
            };

        trace[mem_cols::COL_MEM_DIFF_CLK][i] =
            if i == 0 || trace[mem_cols::COL_MEM_DIFF_ADDR][i] != F::ZERO {
                F::ZERO
            } else {
                trace[mem_cols::COL_MEM_CLK][i] - trace[mem_cols::COL_MEM_CLK][i - 1]
            };
    }

    // If the trace length is not a power of two, we need to extend the trace to the
    // next power of two. The additional elements are filled with the last row
    // of the trace.
    trace = pad_mem_trace(trace);

    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            mem_cols::NUM_MEM_COLS,
            v.len()
        )
    })
}

#[cfg(test)]
mod test {
    use plonky2::hash::hash_types::RichField;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::memory::columns as mem_cols;
    use crate::memory::test_utils::memory_trace_test_case;

    // PADDING  ADDR  CLK   OP  VALUE  DIFF_ADDR  DIFF_ADDR_INV        DIFF_CLK
    // 0        100   0     SB  5      0          0                    0
    // 0        100   1     LB  5      0          0                    4
    // 0        100   4     SB  10     0          0                    12
    // 0        100   5     LB  10     0          0                    4
    // 0        200   2     SB  15     100        3504881373188771021  0
    // 0        200   3     LB  15     0          0                    4
    // 1        200   3     LB  15     0          0                    4
    // 1        200   3     LB  15     0          0                    4
    fn expected_trace<F: RichField>() -> [Vec<F>; mem_cols::NUM_MEM_COLS] {
        [
            vec![
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ONE,
                F::ONE,
            ],
            // ADDR
            vec![
                F::from_canonical_u32(100),
                F::from_canonical_u32(100),
                F::from_canonical_u32(100),
                F::from_canonical_u32(100),
                F::from_canonical_u32(200),
                F::from_canonical_u32(200),
                F::from_canonical_u32(200),
                F::from_canonical_u32(200),
            ],
            // CLK
            vec![
                F::from_canonical_u32(0),
                F::from_canonical_u32(1),
                F::from_canonical_u32(4),
                F::from_canonical_u32(5),
                F::from_canonical_u32(2),
                F::from_canonical_u32(3),
                F::from_canonical_u32(3),
                F::from_canonical_u32(3),
            ],
            // OP
            vec![
                F::ONE,
                F::ZERO,
                F::ONE,
                F::ZERO,
                F::ONE,
                F::ZERO,
                F::ZERO,
                F::ZERO,
            ],
            // VALUE
            vec![
                F::from_canonical_u32(5),
                F::from_canonical_u32(5),
                F::from_canonical_u32(10),
                F::from_canonical_u32(10),
                F::from_canonical_u32(15),
                F::from_canonical_u32(15),
                F::from_canonical_u32(15),
                F::from_canonical_u32(15),
            ],
            // DIFF_ADDR
            vec![
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::from_canonical_u32(100),
                F::ZERO,
                F::ZERO,
                F::ZERO,
            ],
            // DIFF_ADDR_INV
            vec![
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::from_canonical_u64(3_504_881_373_188_771_021),
                F::ZERO,
                F::ZERO,
                F::ZERO,
            ],
            // DIFF_CLK
            vec![
                F::from_canonical_u32(0),
                F::from_canonical_u32(1),
                F::from_canonical_u32(3),
                F::from_canonical_u32(1),
                F::from_canonical_u32(0),
                F::from_canonical_u32(1),
                F::from_canonical_u32(0),
                F::from_canonical_u32(0),
            ],
        ]
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

        let indices = vec![0, 1, 4, 5];
        let expected_trace_vec: Vec<Vec<F>> = expected_trace()
            .iter()
            .map(|v| indices.iter().filter_map(|&i| v.get(i).copied()).collect())
            .collect();

        let expected_trace: [Vec<F>; mem_cols::NUM_MEM_COLS] =
            expected_trace_vec.try_into().expect("Mismatched lengths");

        assert_eq!(trace, expected_trace);
    }
}
