use std::collections::BTreeSet;

use mozak_runner::elf::Program;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use super::memoryinit::generate_memory_init_trace;
use crate::memory_zeroinit::columns::MemoryZeroInit;
use crate::utils::pad_trace_with_default;

#[must_use]
pub(crate) fn init_in_program<F: RichField>(program: &Program) -> BTreeSet<u32> {
    generate_memory_init_trace::<F>(program)
        .iter()
        .filter(|row| row.filter.is_one())
        .filter_map(|row| row.address.to_noncanonical_u64().try_into().ok())
        .collect()
}

#[must_use]
pub(crate) fn used_in_execution<F: RichField>(step_rows: &[Row<F>]) -> BTreeSet<u32> {
    step_rows
        .iter()
        .flat_map(|row| row.aux.mem_addresses_used.clone())
        // Our constraints require that we start at memory address 0 and end at u32::MAX,
        // so we always consider these two used.  (This saves rangechecking the addresses
        // themselves, we only rangecheck their difference.)
        .chain([0, u32::MAX])
        .collect()
}

/// Generates a zero init trace
#[must_use]
pub fn generate_memory_zero_init_trace<F: RichField>(
    step_rows: &[Row<F>],
    program: &Program,
) -> Vec<MemoryZeroInit<F>> {
    let init_in_program: BTreeSet<u32> = init_in_program::<F>(program);
    let used_in_execution: BTreeSet<u32> = used_in_execution(step_rows);
    let trace: Vec<_> = used_in_execution
        .difference(&init_in_program)
        .map(|&addr| MemoryZeroInit {
            addr: F::from_canonical_u32(addr),
            filter: F::ONE,
        })
        .collect();

    log::trace!("MemoryZeroInit trace length: {:?}", trace.len());
    pad_trace_with_default(trace)
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

    use super::*;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::test_utils::prep_table;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn generate_trace() {
        let (program, record) = memory_trace_test_case(1);
        let trace = generate_memory_zero_init_trace::<F>(&record.executed, &program);

        assert_eq!(
            trace,
            // In `memory_trace_test_case()`, there is 1 operation each on addresses
            // '100' and '200' that only happen upon execution that is not in
            // `MemoryInit`. This is tracked in this trace here, to prep for CTL.
            prep_table(vec![
                // addr, filter
                [0, 1],
                [100, 1],
                [200, 1],
                [u64::from(u32::MAX), 1],
                // padding
                [0, 0],
                [0, 0],
                [0, 0],
                [0, 0],
            ])
        );
    }
}
