use itertools::{self, Itertools};
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::memory::trace::get_memory_inst_clk;
use crate::memory_halfword::columns::{HalfWordMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<HalfWordMemory<F>>) -> Vec<HalfWordMemory<F>> {
    trace.resize(
        trace.len().next_power_of_two().max(MIN_TRACE_LENGTH),
        HalfWordMemory {
            // Some columns need special treatment..
            ops: Ops::default(),
            // .. and all other columns just have their last value duplicated.
            ..trace.last().copied().unwrap_or_default()
        },
    );
    trace
}

/// Filter the memory trace to only include halfword load and store
/// instructions.
pub fn filter_memory_trace<F: RichField>(step_rows: &[Row<F>]) -> impl Iterator<Item = &Row<F>> {
    step_rows
        .iter()
        .filter(|row| matches!(row.instruction.op, Op::LH | Op::LHU | Op::SH))
}

#[must_use]
pub fn generate_halfword_memory_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<HalfWordMemory<F>> {
    pad_mem_trace(
        filter_memory_trace(step_rows)
            .map(|s| {
                let op = s.instruction.op;
                let mem_addr0 = s.aux.mem.unwrap_or_default().addr;
                let mem_addr1 = mem_addr0.wrapping_add(1);
                HalfWordMemory {
                    clk: get_memory_inst_clk(s),
                    addrs: [
                        F::from_canonical_u32(mem_addr0),
                        F::from_canonical_u32(mem_addr1),
                    ],
                    ops: Ops {
                        is_store: F::from_bool(matches!(op, Op::SH)),
                        is_load: F::from_bool(matches!(op, Op::LH | Op::LHU)),
                    },
                    limbs: [
                        F::from_canonical_u32(s.aux.dst_val & 0xFF),
                        F::from_canonical_u32((s.aux.dst_val >> 8) & 0xFF),
                    ],
                }
            })
            .collect_vec(),
    )
}

#[cfg(test)]
mod tests {

    use plonky2::field::goldilocks_field::GoldilocksField;

    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
    };
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::memory_halfword::test_utils::halfword_memory_trace_test_case;
    use crate::test_utils::prep_table;

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SH) and load byte signed / unsigned (LH/LHU)
    // operations to memory and then checks if the memory trace is generated
    // correctly.
    #[test]
    #[rustfmt::skip]
    fn generate_half_memory_trace() {
        let (program, record) = halfword_memory_trace_test_case(1);

        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
        let io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
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
        assert_eq!(trace,
            prep_table(vec![
                //is_writable  addr   clk  is_store, is_load, is_init  value  diff_clk
                [       1,     400,   1,      0,        0,       1,        0,        0],  // Memory Init: 400
                [       1,     400,   2,      1,        0,       0,        2,        1],  // Operations:  400
                [       1,     400,   3,      0,        1,       0,        2,        1],  // Operations:  400
                [       1,     401,   1,      0,        0,       1,        0,        0],  // Memory Init: 401
                [       1,     401,   2,      1,        0,       0,        1,        1],  // Operations:  401
                [       1,     401,   3,      0,        1,       0,        1,        1],  // Operations:  401
                [       1,     402,   1,      0,        0,       1,        0,        0],  // Memory Init: 402
                [       1,     403,   1,      0,        0,       1,        0,        0],  // Memory Init: 403
                [       1,     500,   1,      0,        0,       1,        0,        0],  // Memory Init: 500
                [       1,     500,   4,      1,        0,       0,        4,        3],  // Operations:  500
                [       1,     500,   5,      0,        1,       0,        4,        1],  // Operations:  500
                [       1,     501,   1,      0,        0,       1,        0,        0],  // Memory Init: 501
                [       1,     501,   4,      1,        0,       0,        3,        3],  // Operations:  501
                [       1,     501,   5,      0,        1,       0,        3,        1],  // Operations:  501
                [       1,     502,   1,      0,        0,       1,        0,        0],  // Memory Init: 502
                [       1,     502,   1,      0,        0,       0,        0,        0],  // padding
            ])
        );
    }
}
