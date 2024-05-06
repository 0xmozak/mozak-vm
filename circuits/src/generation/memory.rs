use std::collections::HashSet;

use itertools::chain;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::memory::columns::Memory;
use crate::memory::trace::{get_memory_inst_addr, get_memory_inst_clk, get_memory_raw_value};
use crate::memory_fullword::columns::FullWordMemory;
use crate::memory_halfword::columns::HalfWordMemory;
use crate::memory_zeroinit::columns::MemoryZeroInit;
use crate::memoryinit::columns::MemoryInit;
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytes;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::storage_device::columns::StorageDevice;

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<Memory<F>>) -> Vec<Memory<F>> {
    trace.resize(
        // We need to pad by at least one, because our constraints require at least one dummy row
        // at the end.
        (trace.len() + 1).next_power_of_two().max(MIN_TRACE_LENGTH),
        Memory {
            // Some columns need special treatment..
            is_store: F::ZERO,
            is_load: F::ZERO,
            is_init: F::ZERO,
            // .. and all other columns just have their last value duplicated.
            ..trace.last().copied().unwrap_or_default()
        },
    );
    trace
}

/// Generates Memory trace from dynamic VM execution of
/// `Program`. These need to be further interleaved with
/// static memory trace generated from `Program` for final
/// execution for final memory trace.
pub fn generate_memory_trace_from_execution<F: RichField>(
    step_rows: &[Row<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    step_rows
        .iter()
        .filter(|row| {
            row.aux.mem.is_some() && matches!(row.instruction.op, Op::LB | Op::LBU | Op::SB)
        })
        .map(|row| {
            let addr: F = get_memory_inst_addr(row);
            let op = row.instruction.op;
            Memory {
                addr,
                clk: get_memory_inst_clk(row),
                is_store: F::from_bool(matches!(op, Op::SB)),
                is_load: F::from_bool(matches!(op, Op::LB | Op::LBU)),
                is_init: F::ZERO,
                value: get_memory_raw_value(row),

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

/// Generates Memory trace from a memory zero init table.
///
/// These need to be further interleaved with runtime memory trace generated
/// from VM execution for final memory trace.
pub fn transform_memory_zero_init<F: RichField>(
    memory_init_rows: &[MemoryZeroInit<F>],
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

pub fn transform_poseidon2_sponge<F: RichField>(
    sponge_data: &[Poseidon2Sponge<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    sponge_data.iter().flat_map(Into::<Vec<Memory<F>>>::into)
}

pub fn transform_poseidon2_output_bytes<F: RichField>(
    output_bytes: &[Poseidon2OutputBytes<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    output_bytes.iter().flat_map(Into::<Vec<Memory<F>>>::into)
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

/// Generates Memory trace from a memory io table.
///
/// These need to be further interleaved with runtime memory trace generated
/// from VM execution for final memory trace.
pub fn transform_storage<F: RichField>(
    storage_devices: &[StorageDevice<F>],
) -> impl Iterator<Item = Memory<F>> + '_ {
    storage_devices.iter().filter_map(Option::<Memory<F>>::from)
}

fn key<F: RichField>(memory: &Memory<F>) -> (u64, u64) {
    (
        memory.addr.to_canonical_u64(),
        memory.clk.to_canonical_u64(),
    )
}

/// Generates memory trace using static component `program` for memory
/// initialization and dynamic component `step_rows` for access (load and store)
/// of memory elements.
/// Trace constraints are supposed to abide by read-only and read-write address
/// constraints.
/// Merge different types of memory traces in to one [Memory] trace
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn generate_memory_trace<F: RichField>(
    step_rows: &[Row<F>],
    memory_init_rows: &[MemoryInit<F>],
    memory_zeroinit_rows: &[MemoryZeroInit<F>],
    halfword_memory_rows: &[HalfWordMemory<F>],
    fullword_memory_rows: &[FullWordMemory<F>],
    private_tape_rows: &[StorageDevice<F>],
    public_tape_rows: &[StorageDevice<F>],
    call_tape_rows: &[StorageDevice<F>],
    event_tape_rows: &[StorageDevice<F>],
    events_commitment_tape_rows: &[StorageDevice<F>],
    castlist_commitment_tape_rows: &[StorageDevice<F>],
    poseidon2_sponge_rows: &[Poseidon2Sponge<F>],
    poseidon2_output_bytes_rows: &[Poseidon2OutputBytes<F>],
) -> Vec<Memory<F>> {
    // `merged_trace` is address sorted combination of static and
    // dynamic memory trace components of program (ELF and execution)
    // `merge` operation is expected to be stable
    let mut merged_trace: Vec<Memory<F>> = chain!(
        transform_memory_init::<F>(memory_init_rows),
        transform_memory_zero_init(memory_zeroinit_rows),
        generate_memory_trace_from_execution(step_rows),
        transform_halfword(halfword_memory_rows),
        transform_fullword(fullword_memory_rows),
        transform_storage(private_tape_rows),
        transform_storage(public_tape_rows),
        transform_storage(call_tape_rows),
        transform_storage(event_tape_rows),
        transform_storage(events_commitment_tape_rows),
        transform_storage(castlist_commitment_tape_rows),
        transform_poseidon2_sponge(poseidon2_sponge_rows),
        transform_poseidon2_output_bytes(poseidon2_output_bytes_rows,),
    )
    .collect();

    let read_only_addresses: HashSet<F> = memory_init_rows
        .iter()
        .filter(|row| row.filter.is_nonzero() && row.is_writable.is_zero())
        .map(|row| row.address)
        .collect();

    merged_trace.sort_by_key(key);
    let merged_trace: Vec<_> = merged_trace
        .into_iter()
        .map(|mem| Memory {
            is_writable: F::from_bool(!read_only_addresses.contains(&mem.addr)),
            ..mem
        })
        .collect();

    log::trace!("trace {:?}", merged_trace);
    pad_mem_trace(merged_trace)
}

#[cfg(test)]
mod tests {
    use im::hashmap::HashMap;
    use mozak_runner::elf::{Data, Program};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use super::pad_mem_trace;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::generation::storage_device::{
        generate_call_tape_trace, generate_cast_list_commitment_tape_trace,
        generate_event_tape_trace, generate_events_commitment_tape_trace,
        generate_private_tape_trace, generate_public_tape_trace,
    };
    use crate::memory::columns::Memory;
    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{fast_test_config, prep_table};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = MemoryStark<F, D>;

    #[rustfmt::skip]
    #[test]
    #[ignore]
    #[should_panic = "Constraint failed in"]
    // TODO(Roman): fix this test, looks like we should constrain the `is_init`
    /// Test that we have a constraint to catch, if there is no init for any memory address.
    fn no_init() {
        let _ = env_logger::try_init();
        let stark = S::default();

        let trace: Vec<Memory<GoldilocksField>> = prep_table(vec![
            //is_writable  addr  clk is_store, is_load, is_init  value
            [       0,     100,   1,     0,      0,       0,        1],
            [       1,     100,   1,     0,      0,       0,        2],
        ]);
        let trace = pad_mem_trace(trace);
        let trace_poly_values = trace_rows_to_poly_values(trace);
        let config = fast_test_config();
        // This will fail, iff debug assertions are enabled.
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        ).unwrap();
        assert!(verify_stark_proof(stark, proof, &config).is_ok(), "failing constraint: init is required per memory address");
    }

    // TODO(Matthias): restore the test that shows that double-init is not allowed.
    // The complication is that this is now caught by a range-check on the address
    // difference, not by direct constraints.

    // This test simulates the scenario of a set of instructions
    // which perform store byte (SB) and load byte unsigned (LBU) operations
    // to memory and then checks if the memory trace is generated correctly.
    #[test]
    #[rustfmt::skip]
    fn generate_memory_trace() {
        let (program, record) = memory_trace_test_case(1);

        let memory_init = generate_memory_init_trace(&program);
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, &program);

        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let private_tape_rows = generate_private_tape_trace(&record.executed);
        let public_tape_rows = generate_public_tape_trace(&record.executed);

        let call_tape_rows = generate_call_tape_trace(&record.executed);
        let event_tape_rows = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows = generate_cast_list_commitment_tape_trace(&record.executed);
        let poseidon2_sponge_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_sponge_trace);

        let trace = super::generate_memory_trace::<GoldilocksField>(
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
            &poseidon2_sponge_trace,
            &poseidon2_output_bytes,
        );
        let last = u64::from(u32::MAX);
        assert_eq!(
            trace,
            prep_table(vec![
                //is_writable  addr  clk is_store, is_load, is_init  value
                [       1,     0,     0,     0,      0,       1,     0],  // Memory Init: 0
                [       1,     100,   0,     0,      0,       1,     0],  // Zero Init:   100
                [       1,     100,   2,     1,      0,       0,   255],  // Operations:  100
                [       1,     100,   3,     0,      1,       0,   255],  // Operations:  100
                [       1,     100,   6,     1,      0,       0,    10],  // Operations:  100
                [       1,     100,   7,     0,      1,       0,    10],  // Operations:  100
                [       1,     101,   1,     0,      0,       1,     0],  // Memory Init: 101
                [       1,     102,   1,     0,      0,       1,     0],  // Memory Init: 102
                [       1,     103,   1,     0,      0,       1,     0],  // Memory Init: 103
                [       1,     200,   0,     0,      0,       1,     0],  // Zero Init:   200
                [       1,     200,   4,     1,      0,       0,    15],  // Operations:  200
                [       1,     200,   5,     0,      1,       0,    15],  // Operations:  200
                [       1,     201,   1,     0,      0,       1,     0],  // Memory Init: 201
                [       1,     202,   1,     0,      0,       1,     0],  // Memory Init: 202
                [       1,     203,   1,     0,      0,       1,     0],  // Memory Init: 203
                [       1,    last,   0,     0,      0,       1,     0],  // Memory Init: last
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
                [       1,    last,   0,     0,      0,       0,     0],  // padding
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
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&[], &program);

        let halfword_memory = generate_halfword_memory_trace(&[]);
        let fullword_memory = generate_fullword_memory_trace(&[]);
        let private_tape_rows = generate_private_tape_trace(&[]);
        let public_tape_rows = generate_public_tape_trace(&[]);
        let call_tape_rows = generate_call_tape_trace(&[]);
        let event_tape_rows = generate_call_tape_trace(&[]);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&[]);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&[]);
        let poseidon2_trace = generate_poseidon2_sponge_trace(&[]);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_trace);
        let trace = super::generate_memory_trace::<F>(
            &[],
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
            &poseidon2_trace,
            &poseidon2_output_bytes,
        );

        let last = u64::from(u32::MAX);
        assert_eq!(trace, prep_table(vec![
            // is_writable   addr   clk  is_store, is_load, is_init  value
            [        1,        0,    0,     0,        0,      1,         0],  // Memory Init: 0
            [        0,      100,   1,      0,        0,      1,         5],
            [        0,      101,   1,      0,        0,      1,         6],
            [        1,      200,   1,      0,        0,      1,         7],
            [        1,      201,   1,      0,        0,      1,         8],
            [        1,     last,   0,      0,        0,      1,         0],  // Memory Init: last
            [        1,     last,   0,      0,        0,      0,         0],  // padding
            [        1,     last,   0,      0,        0,      0,         0],  // padding
        ]));
    }
}
