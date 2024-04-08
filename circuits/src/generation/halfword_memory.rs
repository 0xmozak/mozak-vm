use itertools::Itertools;
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

    use mozak_runner::elf::Program;
    use mozak_runner::instruction::Op::{LH, LHU, SH};
    use mozak_runner::instruction::{Args, Instruction};
    use mozak_runner::util::execute_code;
    use mozak_runner::vm::ExecutionRecord;
    use plonky2::field::goldilocks_field::GoldilocksField;

    use crate::generation::generate_poseidon2_output_bytes_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
    };
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::ops;
    use crate::test_utils::{inv, prep_table};

    // TODO(Matthias): Consider unifying with the byte memory example?
    #[must_use]
    fn halfword_memory_trace_test_case(
        repeats: usize,
    ) -> (Program, ExecutionRecord<GoldilocksField>) {
        let new = Instruction::new;
        let instructions = [
            new(SH, Args {
                // addr = rs2 + imm, value = rs1-value
                // store-full-word of address = 100, value 0x0102
                rs1: 1,
                imm: 400,
                ..Args::default()
            }),
            new(LH, Args {
                // addr = rs2 + imm, value = rd-value
                // load-full-word from address = 100 to reg-3, value of 0x0102
                rd: 3,
                imm: 400,
                ..Args::default()
            }),
            new(SH, Args {
                // addr = rs2 + imm, value = rs1
                // store-full-word of address = 200, value 0x0304
                rs1: 2,
                imm: 500,
                ..Args::default()
            }),
            new(LHU, Args {
                // addr = rs2 + imm, value = rd
                // load-full-word from address = 200 to reg-4, value of 0x0304
                rd: 4,
                imm: 500,
                ..Args::default()
            }),
        ];
        let code = std::iter::repeat(&instructions)
            .take(repeats)
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let (program, record) = execute_code(
            code,
            &[
                (400, 0),
                (401, 0),
                (402, 0),
                (403, 0),
                (500, 0),
                (501, 0),
                (502, 0),
            ],
            &[(1, 0x0102), (2, 0x0304), (3, 0xFFFF), (4, 0x0000_FFFF)],
        );

        if repeats > 0 {
            let state = &record.last_state;
            assert_eq!(state.load_u32(400), 0x0102);
            assert_eq!(state.get_register_value(3), 0x0102);
            assert_eq!(state.load_u32(500), 0x0304);
            assert_eq!(state.get_register_value(4), 0x0304);
        }
        (program, record)
    }

    type F = GoldilocksField;
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
        let store_word_rows = ops::sw::generate(&record.executed);
        let load_word_rows = ops::lw::generate(&record.executed);
        let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
        let io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
        let poseidon2_rows = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_rows);

        let trace = generate_memory_trace::<GoldilocksField>(
            &record.executed,
            &memory_init,
            &halfword_memory,
            &store_word_rows,
            &load_word_rows,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &poseidon2_rows,
            &poseidon2_output_bytes,
        );
        assert_eq!(trace,
            prep_table(vec![
                //is_writable  addr   clk  is_store, is_load, is_init  value  diff_clk      diff_addr_inv
                [       1,     400,   1,      0,        0,       1,        0,        0,     inv::<F>(400)],// Memory Init: 400
                [       1,     400,   2,      1,        0,       0,        2,        1,     inv::<F>(0)],  // Operations:  400
                [       1,     400,   3,      0,        1,       0,        2,        1,     inv::<F>(0)],  // Operations:  400
                [       1,     401,   1,      0,        0,       1,        0,        0,     inv::<F>(1)],  // Memory Init: 401
                [       1,     401,   2,      1,        0,       0,        1,        1,     inv::<F>(0)],  // Operations:  401
                [       1,     401,   3,      0,        1,       0,        1,        1,     inv::<F>(0)],  // Operations:  401
                [       1,     402,   1,      0,        0,       1,        0,        0,     inv::<F>(1)],  // Memory Init: 402
                [       1,     403,   1,      0,        0,       1,        0,        0,     inv::<F>(1)],  // Memory Init: 403
                [       1,     500,   1,      0,        0,       1,        0,        0,     inv::<F>(97)], // Memory Init: 500
                [       1,     500,   4,      1,        0,       0,        4,        3,     inv::<F>(0)],  // Operations:  500
                [       1,     500,   5,      0,        1,       0,        4,        1,     inv::<F>(0)],  // Operations:  500
                [       1,     501,   1,      0,        0,       1,        0,        0,     inv::<F>(1)],  // Memory Init: 501
                [       1,     501,   4,      1,        0,       0,        3,        3,     inv::<F>(0)],  // Operations:  501
                [       1,     501,   5,      0,        1,       0,        3,        1,     inv::<F>(0)],  // Operations:  501
                [       1,     502,   1,      0,        0,       1,        0,        0,     inv::<F>(1)],  // Memory Init: 502
                [       1,     502,   1,      0,        0,       0,        0,        0,     inv::<F>(0)],  // padding
            ])
        );
    }
}
