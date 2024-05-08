use itertools::Itertools;
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
    use mozak_runner::code;
    use mozak_runner::elf::Program;
    use mozak_runner::instruction::Op::{LW, SW};
    use mozak_runner::instruction::{Args, Instruction};
    use mozak_runner::vm::ExecutionRecord;
    use plonky2::field::goldilocks_field::GoldilocksField;

    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::storage_device::{
        generate_call_tape_trace, generate_cast_list_commitment_tape_trace,
        generate_event_tape_trace, generate_events_commitment_tape_trace,
        generate_private_tape_trace, generate_public_tape_trace,
    };
    use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::test_utils::prep_table;

    // TODO(Matthias): Consider unifying with the byte memory example?
    #[must_use]
    pub fn fullword_memory_trace_test_case(
        repeats: usize,
    ) -> (Program, ExecutionRecord<GoldilocksField>) {
        let new = Instruction::new;
        let instructions = [
            new(SW, Args {
                // addr = rs2 + imm, value = rs1-value
                // store-full-word of address = 100, value 0x0a0b_0c0d
                rs1: 1,
                imm: 600,
                ..Args::default()
            }),
            new(LW, Args {
                // addr = rs2 + imm, value = rd-value
                // load-full-word from address = 100 to reg-3, value of 0x0a0b_0c0d
                rd: 3,
                imm: 600,
                ..Args::default()
            }),
            new(SW, Args {
                // addr = rs2 + imm, value = rs1
                // store-full-word of address = 200, value 0x0102_0304
                rs1: 2,
                imm: 700,
                ..Args::default()
            }),
            new(LW, Args {
                // addr = rs2 + imm, value = rd
                // load-full-word from address = 200 to reg-4, value of 0x0102_0304
                rd: 4,
                imm: 700,
                ..Args::default()
            }),
        ];
        let code = std::iter::repeat(&instructions)
            .take(repeats)
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let (program, record) = code::execute(
            code,
            &[
                (600, 0),
                (601, 0),
                (602, 0),
                (603, 0),
                (700, 0),
                (701, 0),
                (702, 0),
                (703, 0),
            ],
            &[
                (1, 0x0a0b_0c0d),
                (2, 0x0102_0304),
                (3, 0xFFFF),
                (4, 0x0000_FFFF),
            ],
        );

        if repeats > 0 {
            let state = &record.last_state;
            assert_eq!(state.load_u32(600), 0x0a0b_0c0d);
            assert_eq!(state.get_register_value(3), 0x0a0b_0c0d);
            assert_eq!(state.load_u32(700), 0x0102_0304);
            assert_eq!(state.get_register_value(4), 0x0102_0304);
        }
        (program, record)
    }

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte unsigned (LBU) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    #[rustfmt::skip]
    fn generate_full_memory_trace() {
        let (program, record) = fullword_memory_trace_test_case(1);

        let memory_init = generate_memory_init_trace(&program);
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, &program);

        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let private_tape_rows = generate_private_tape_trace(&record.executed);
        let public_tape_rows= generate_public_tape_trace(&record.executed);
        let call_tape_rows = generate_call_tape_trace(&record.executed);
        let event_tape_rows = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&record.executed);
        let poseidon2_rows = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_rows);
        let trace = generate_memory_trace::<GoldilocksField>(
            &record.executed,
            &memory_init,
            &memory_zeroinit_rows,
            &halfword_memory,
            &fullword_memory,
            &private_tape_rows,
            &public_tape_rows,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &poseidon2_rows,
            &poseidon2_output_bytes,
        );
        let last = u64::from(u32::MAX);
        assert_eq!(
            trace,
            prep_table(vec![
                //is_writable  addr   clk  is_store, is_load, is_init  value
                [       1,     0,     0,     0,         0,       1,        0],  // Memory Init: 0
                [       1,     600,   1,     0,         0,       1,        0],  // Memory Init: 600
                [       1,     600,   2,     1,         0,       0,       13],  // Operations:  600
                [       1,     600,   3,     0,         1,       0,       13],  // Operations:  600
                [       1,     601,   1,     0,         0,       1,        0],  // Memory Init: 601
                [       1,     601,   2,     1,         0,       0,       12],  // Operations:  601
                [       1,     601,   3,     0,         1,       0,       12],  // Operations:  601
                [       1,     602,   1,     0,         0,       1,        0],  // Memory Init: 602
                [       1,     602,   2,     1,         0,       0,       11],  // Operations:  602
                [       1,     602,   3,     0,         1,       0,       11],  // Operations:  603
                [       1,     603,   1,     0,         0,       1,        0],  // Memory Init: 603
                [       1,     603,   2,     1,         0,       0,       10],  // Operations:  603
                [       1,     603,   3,     0,         1,       0,       10],  // Operations:  603
                [       1,     700,   1,     0,         0,       1,        0],  // Memory Init: 700
                [       1,     700,   4,     1,         0,       0,        4],  // Operations:  700
                [       1,     700,   5,     0,         1,       0,        4],  // Operations:  700
                [       1,     701,   1,     0,         0,       1,        0],  // Memory Init: 701
                [       1,     701,   4,     1,         0,       0,        3],  // Operations:  701
                [       1,     701,   5,     0,         1,       0,        3],  // Operations:  701
                [       1,     702,   1,     0,         0,       1,        0],  // Memory Init: 702
                [       1,     702,   4,     1,         0,       0,        2],  // Operations:  702
                [       1,     702,   5,     0,         1,       0,        2],  // Operations:  703
                [       1,     703,   1,     0,         0,       1,        0],  // Memory Init: 703
                [       1,     703,   4,     1,         0,       0,        1],  // Operations:  703
                [       1,     703,   5,     0,         1,       0,        1],  // Operations:  703
                [       1,    last,   0,     0,         0,       1,        0],  // Memory Init: last
                [       1,    last,   0,     0,         0,       0,        0],  // padding
                [       1,    last,   0,     0,         0,       0,        0],  // padding
                [       1,    last,   0,     0,         0,       0,        0],  // padding
                [       1,    last,   0,     0,         0,       0,        0],  // padding
                [       1,    last,   0,     0,         0,       0,        0],  // padding
                [       1,    last,   0,     0,         0,       0,        0],  // padding
            ])
        );
    }
}
