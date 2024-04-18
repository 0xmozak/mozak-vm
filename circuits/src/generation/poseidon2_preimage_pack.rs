use plonky2::hash::hash_types::RichField;

use crate::poseidon2_preimage_pack::columns::Poseidon2PreimagePack;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::utils::pad_trace_with_default;

pub fn generate_poseidon2_preimage_pack_trace<F: RichField>(
    poseidon2_sponge_rows: &[Poseidon2Sponge<F>],
) -> Vec<Poseidon2PreimagePack<F>> {
    let trace: Vec<Poseidon2PreimagePack<F>> = poseidon2_sponge_rows
        .iter()
        .flat_map(Into::<Vec<Poseidon2PreimagePack<F>>>::into)
        .collect();
    pad_trace_with_default(trace)
}

#[cfg(test)]
mod tests {
    use mozak_runner::vm::Row;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::generation::MIN_TRACE_LENGTH;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    #[test]
    fn generate_poseidon2_preimage_pack_trace() {
        let data = "😇 Mozak is knowledge arguments based technology".to_string();
        let input_start_addr = 1024;
        let output_start_addr = 2048;
        let (_program, record) = create_poseidon2_test(&[Poseidon2Test {
            data,
            input_start_addr,
            output_start_addr,
        }]);

        let step_rows = record.executed;

        let sponge_trace = generate_poseidon2_sponge_trace(&step_rows);
        let trace = super::generate_poseidon2_preimage_pack_trace(&sponge_trace);
        // for one sponge construct we have one row with gen_output = 1.
        // So we expect other padding data to make trace of len MIN_TRACE_LENGTH.
        assert_eq!(trace.len(), MIN_TRACE_LENGTH);
    }

    #[test]
    fn generate_poseidon2_trace_with_dummy() {
        let step_rows: Vec<Row<F>> = vec![];
        let sponge_trace = generate_poseidon2_sponge_trace(&step_rows);
        let trace = super::generate_poseidon2_preimage_pack_trace(&sponge_trace);
        assert_eq!(trace.len(), MIN_TRACE_LENGTH);
    }
}
