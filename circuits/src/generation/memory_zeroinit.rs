use std::collections::HashSet;

use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory_zeroinit::columns::MemoryZeroInit;
use crate::memoryinit::columns::MemoryInit;
use crate::utils::pad_trace_with_default;

/// Generates a zero init trace
#[must_use]
pub fn generate_memory_zero_init_trace<F: RichField>(
    mem_init_rows: &[MemoryInit<F>],
    step_rows: &[Row<F>],
) -> Vec<MemoryZeroInit<F>> {
    let mut zeroinit_set: HashSet<F> = HashSet::new();
    let meminit_map: HashSet<F> = mem_init_rows.iter().map(|r| r.element.address).collect();

    step_rows
        .iter()
        .filter(|row| {
            row.aux.mem.is_some()
                && matches!(
                    row.instruction.op,
                    Op::LB | Op::LBU | Op::SB | Op::SH | Op::LH | Op::LHU | Op::LW | Op::SW
                )
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

    let memory_zeroinits: Vec<MemoryZeroInit<F>> = zeroinit_set
        .into_iter()
        .map(|addr| MemoryZeroInit {
            addr,
            filter: F::ONE,
        })
        .collect();

    pad_trace_with_default(memory_zeroinits)
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use super::*;
    use crate::generation::memoryinit::generate_memory_init_trace;
    use crate::memory::test_utils::memory_trace_test_case;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn test_generate() {
        let (program, record) = memory_trace_test_case(1);
        let memory_init_rows = generate_memory_init_trace(&program);
        let _ = generate_memory_zero_init_trace::<F>(&memory_init_rows, &record.executed);
    }
}
