use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::trace::get_memory_inst_clk;
use crate::memory_halfword::columns::{HalfWordMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<HalfWordMemory<F>>) -> Vec<HalfWordMemory<F>> {
    trace.resize(trace.len().next_power_of_two(), HalfWordMemory {
        // Some columns need special treatment..
        ops: Ops::default(),
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Filter the memory trace to only include halfword load and store
/// instructions.
pub fn filter_memory_trace<'a, F: RichField>(
    program: &'a Program,
    step_rows: &'a [Row<F>],
) -> impl Iterator<Item = &'a Row<F>> {
    step_rows.iter().filter(|row| {
        matches!(
            row.state.current_instruction(program).op,
            Op::LH | Op::LHU | Op::SH
        )
    })
}

#[must_use]
pub fn generate_halfword_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row<F>],
) -> Vec<HalfWordMemory<F>> {
    pad_mem_trace(
        filter_memory_trace(program, step_rows)
            .map(|s| {
                let op = s.state.current_instruction(program).op;
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
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::generate_io_memory_trace;
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::memory_halfword::test_utils::halfword_memory_trace_test_case;
    use crate::test_utils::{inv, prep_table};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SH) and load byte signed / unsigned (LH/LHU)
    // operations to memory and then checks if the memory trace is generated
    // correctly.
    #[test]
    #[rustfmt::skip]
    fn generate_half_memory_trace() {
        let (program, record) = halfword_memory_trace_test_case(1);

        let memory_init = generate_memory_init_trace(&program);
        let halfword_memory = generate_halfword_memory_trace(&program, &record.executed);
        let fullword_memory = generate_fullword_memory_trace(&program, &record.executed);
        let io_memory_rows = generate_io_memory_trace(&program, &record.executed);

        let trace = generate_memory_trace::<GoldilocksField>(
            &program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_rows,
        );
        let inv = inv::<F>;
        assert_eq!(
            trace,
            prep_table(vec![
                //is_writable  addr   clk  is_sb, is_lbu, is_init  value  diff_addr  diff_addr_inv  diff_clk
                [       1,     400,   0,     0,     0,       1,        0,    400,     inv(400),            0],  // Memory Init: 400
                [       1,     400,   1,     1,     0,       0,        2,      0,           0,             1],  // Operations:  400
                [       1,     400,   2,     0,     1,       0,        2,      0,           0,             1],  // Operations:  400
                [       1,     401,   0,     0,     0,       1,        0,      1,       inv(1),            0],  // Memory Init: 401
                [       1,     401,   1,     1,     0,       0,        1,      0,           0,             1],  // Operations:  401
                [       1,     401,   2,     0,     1,       0,        1,      0,           0,             1],  // Operations:  401
                [       1,     402,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 402
                [       1,     403,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 403
                [       1,     500,   0,     0,     0,       1,        0,     97,     inv(97),             0],  // Memory Init: 500
                [       1,     500,   3,     1,     0,       0,        4,      0,           0,             3],  // Operations:  500
                [       1,     500,   4,     0,     1,       0,        4,      0,           0,             1],  // Operations:  500
                [       1,     501,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 501
                [       1,     501,   3,     1,     0,       0,        3,      0,           0,             3],  // Operations:  501
                [       1,     501,   4,     0,     1,       0,        3,      0,           0,             1],  // Operations:  501
                [       1,     502,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 502
                [       1,     503,   0,     0,     0,       1,        0,      1,      inv(1),             0],  // Memory Init: 503
            ])
        );
    }
}
