use itertools::{self, Itertools};
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::memory::trace::get_memory_inst_clk;
use crate::memory_fullword::columns::{FullWordMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<FullWordMemory<F>>) -> Vec<FullWordMemory<F>> {
    trace.resize(
        trace.len().next_power_of_two().max(MIN_TRACE_LENGTH),
        FullWordMemory {
            // Some columns need special treatment..
            ops: Ops::default(),
            // .. and all other columns just have their last value duplicated.
            ..trace.last().copied().unwrap_or_default()
        },
    );
    trace
}

/// Returns the rows with full word memory instructions.
pub fn filter_memory_trace<F: RichField>(step_rows: &[Row<F>]) -> impl Iterator<Item = &Row<F>> {
    step_rows
        .iter()
        .filter(|row| matches!(row.instruction.op, Op::LW | Op::SW))
}

#[must_use]
pub fn generate_fullword_memory_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<FullWordMemory<F>> {
    pad_mem_trace(
        filter_memory_trace(step_rows)
            .map(|s| {
                let op = s.instruction.op;
                let base_addr = s.aux.mem.unwrap_or_default().addr;
                let addrs = (0..4)
                    .map(|i| F::from_canonical_u32(base_addr.wrapping_add(i)))
                    .collect_vec()
                    .try_into()
                    .unwrap();
                let limbs = s
                    .aux
                    .dst_val
                    .to_le_bytes()
                    .into_iter()
                    .map(F::from_canonical_u8)
                    .collect_vec()
                    .try_into()
                    .unwrap();
                FullWordMemory {
                    clk: get_memory_inst_clk(s),
                    addrs,
                    ops: Ops {
                        is_store: F::from_bool(matches!(op, Op::SW)),
                        is_load: F::from_bool(matches!(op, Op::LW)),
                    },
                    limbs,
                }
            })
            .collect_vec(),
    )
}
#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
    };
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::memory_fullword::test_utils::fullword_memory_trace_test_case;
    use crate::test_utils::{inv, prep_table};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte unsigned (LBU) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    #[rustfmt::skip]
    fn generate_full_memory_trace() {
        let (program, record) = fullword_memory_trace_test_case(1);

        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
        let io_memory_public_rows= generate_io_memory_public_trace(&record.executed);
        let poseidon2_rows = generate_poseidon2_sponge_trace(&record.executed);
        let trace = generate_memory_trace::<GoldilocksField>(
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &poseidon2_rows,
        );
        let inv = inv::<F>;
        assert_eq!(
            trace,
            prep_table(vec![
                //is_writable  addr   clk  is_store, is_load, is_init  is_zeroed value  diff_addr  diff_addr_inv  diff_clk
                [       1,     600,   0,     0,         0,       1,        0,    600,     inv(600),            0],  // Memory Init: 600
                [       1,     600,   1,     1,         0,       0,       13,      0,           0,             1],  // Operations:  600
                [       1,     600,   2,     0,         1,       0,       13,      0,           0,             1],  // Operations:  600
                [       1,     601,   0,     0,         0,       1,        0,      1,       inv(1),            0],  // Memory Init: 601
                [       1,     601,   1,     1,         0,       0,       12,      0,           0,             1],  // Operations:  601
                [       1,     601,   2,     0,         1,       0,       12,      0,           0,             1],  // Operations:  601
                [       1,     602,   0,     0,         0,       1,        0,      1,      inv(1),             0],  // Memory Init: 602
                [       1,     602,   1,     1,         0,       0,       11,      0,           0,             1],  // Operations:  602
                [       1,     602,   2,     0,         1,       0,       11,      0,           0,             1],  // Operations:  603
                [       1,     603,   0,     0,         0,       1,        0,      1,      inv(1),             0],  // Memory Init: 603
                [       1,     603,   1,     1,         0,       0,       10,      0,           0,             1],  // Operations:  603
                [       1,     603,   2,     0,         1,       0,       10,      0,           0,             1],  // Operations:  603
                [       1,     700,   0,     0,         0,       1,        0,     97,     inv(97),             0],  // Memory Init: 700
                [       1,     700,   3,     1,         0,       0,        4,      0,           0,             3],  // Operations:  700
                [       1,     700,   4,     0,         1,       0,        4,      0,           0,             1],  // Operations:  700
                [       1,     701,   0,     0,         0,       1,        0,      1,      inv(1),             0],  // Memory Init: 701
                [       1,     701,   3,     1,         0,       0,        3,      0,           0,             3],  // Operations:  701
                [       1,     701,   4,     0,         1,       0,        3,      0,           0,             1],  // Operations:  701
                [       1,     702,   0,     0,         0,       1,        0,      1,      inv(1),             0],  // Memory Init: 702
                [       1,     702,   3,     1,         0,       0,        2,      0,           0,             3],  // Operations:  702
                [       1,     702,   4,     0,         1,       0,        2,      0,           0,             1],  // Operations:  703
                [       1,     703,   0,     0,         0,       1,        0,      1,      inv(1),             0],  // Memory Init: 703
                [       1,     703,   3,     1,         0,       0,        1,      0,           0,             3],  // Operations:  703
                [       1,     703,   4,     0,         1,       0,        1,      0,           0,             1],  // Operations:  703
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
                [       1,     703,   4,     0,         0,       0,        1,      0,           0,             0],  // padding
            ])
        );
    }
}
