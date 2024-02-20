use std::collections::HashSet;

use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory_zeroinit::columns::MemoryZeroInit;
use crate::memoryinit::columns::MemoryInit;
use crate::poseidon2_output_bytes::columns::BYTES_COUNT;
use crate::utils::pad_trace_with_default;

/// Generates a zero init trace
#[must_use]
pub fn generate_memory_zero_init_trace<F: RichField>(
    mem_init_rows: &[MemoryInit<F>],
    step_rows: &[Row<F>],
    program: &Program,
) -> Vec<MemoryZeroInit<F>> {
    let mut zeroinit_set: HashSet<F> = HashSet::new();
    let meminit_map: HashSet<F> = mem_init_rows
        .iter()
        .filter_map(|r| {
            if r.filter.is_one() {
                Some(r.element.address)
            } else {
                None
            }
        })
        .collect();

    step_rows
        .iter()
        .filter(|row| {
            // This if-else ensure that mozak-ro-memory addresses are filtered out. No need
            // to insert zero-init rows for mozak-ro-addresses. All this in case
            // mozak-ro-memory indeed exist, because this code should work for a vanilla
            // ELF too

            // Note: here is a brief description how this filtering works:
            // If row related to memory (row.aux.mem.is_some)
            // -> Then:
            //         1) instruction.op is-memory-op
            //         2) If in addition mozak-ro-memory exist - we should filter-it-out ->
            //            check address does NOT belong to it
            // OR ->
            //         1) instruction.op is ECALL
            //         2) row related to poseidon2 (row.aux.poseidon2.is_some)
            (row.aux.mem.is_some()
                && matches!(
                    row.instruction.op,
                    Op::LB | Op::LBU | Op::SB | Op::SH | Op::LH | Op::LHU | Op::LW | Op::SW
                )
                && (if let Some(mrm) = program.mozak_ro_memory.as_ref() {
                    !mrm.is_address_belongs_to_mozak_ro_memory(row.aux.mem.unwrap().addr)
                } else {
                    true
                }))
                || (row.instruction.op == Op::ECALL && row.aux.poseidon2.is_some())
        })
        .for_each(|row| {
            let addr = row.aux.mem.unwrap_or_default().addr;

            let addresses = match row.instruction.op {
                Op::LB | Op::LBU | Op::SB => vec![F::from_canonical_u32(addr)],
                Op::LH | Op::LHU | Op::SH => (0..2)
                    .map(|i| F::from_canonical_u32(addr.wrapping_add(i)))
                    .collect(),
                Op::LW | Op::SW => (0..4)
                    .map(|i| F::from_canonical_u32(addr.wrapping_add(i)))
                    .collect(),
                Op::ECALL => {
                    // must be poseidon2 ECALL as per filter above
                    let output_addr = row.aux.poseidon2.clone().unwrap_or_default().output_addr;
                    (0..u32::try_from(BYTES_COUNT)
                        .expect("BYTES_COUNT of a poseidon output should be representable by a u8"))
                        .map(|i| F::from_canonical_u32(output_addr.wrapping_add(i)))
                        .collect()
                }
                // This should never be reached, because we already filter by memory ops.
                _ => unreachable!(),
            };

            addresses
                .iter()
                .filter(|addr| !meminit_map.contains(addr))
                .for_each(|addr| {
                    zeroinit_set.insert(*addr);
                });
        });

    let mut memory_zeroinits: Vec<MemoryZeroInit<F>> = zeroinit_set
        .into_iter()
        .map(|addr| MemoryZeroInit {
            addr,
            filter: F::ONE,
        })
        .collect();

    memory_zeroinits.sort_by_key(|m| m.addr.to_canonical_u64());
    let trace = pad_trace_with_default(memory_zeroinits);
    log::trace!("MemoryZeroInit trace {:?}", trace);
    trace
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use super::*;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::test_utils::prep_table;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn generate_trace() {
        let (program, record) = memory_trace_test_case(1);
        let memory_init_rows = generate_memory_init_trace(&program);
        let trace =
            generate_memory_zero_init_trace::<F>(&memory_init_rows, &record.executed, &program);

        assert_eq!(
            trace,
            // In `memory_trace_test_case()`, there is 1 operation each on addresses
            // '100' and '200' that only happen upon execution that is not in
            // `MemoryInit`. This is tracked in this trace here, to prep for CTL.
            prep_table(vec![
                // addr, filter
                [100, 1],
                [200, 1],
                // padding
                [0, 0],
                [0, 0],
                [0, 0],
                [0, 0],
                [0, 0],
                [0, 0],
            ])
        );
    }
}
