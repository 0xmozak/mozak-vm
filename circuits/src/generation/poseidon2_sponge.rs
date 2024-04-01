use itertools::Itertools;
use mozak_runner::poseidon2::MozakPoseidon2;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::Poseidon2Permutation;

use crate::generation::MIN_TRACE_LENGTH;
use crate::poseidon2_sponge::columns::{Ops, Poseidon2Sponge};

fn pad_poseidon2_sponge_trace<F: RichField>(
    mut trace: Vec<Poseidon2Sponge<F>>,
) -> Vec<Poseidon2Sponge<F>> {
    trace.resize(
        trace.len().next_power_of_two().max(MIN_TRACE_LENGTH),
        Poseidon2Sponge::default(),
    );
    trace
}

pub fn filter<F: RichField>(step_rows: &[Row<F>]) -> impl Iterator<Item = &Row<F>> {
    step_rows.iter().filter(|row| row.aux.poseidon2.is_some())
}

fn unroll_sponge_data<F: RichField>(row: &Row<F>) -> Vec<Poseidon2Sponge<F>> {
    let poseidon2 = row.aux.poseidon2.clone().expect("please pass filtered row");
    let mut unroll = vec![];
    let rate_size = u32::try_from(Poseidon2Permutation::<F>::RATE).expect("RATE > 2^32");
    assert_eq!(poseidon2.len % rate_size, 0);
    let unroll_count = u32::try_from(poseidon2.sponge_data.len()).expect("too many rows");

    let output_addr = poseidon2.output_addr;
    let mut input_addr = poseidon2.addr;
    let mut input_addr_padded = poseidon2.addr;
    let mut input_len = poseidon2.len;
    for i in 0..unroll_count {
        let ops: Ops<F> = Ops {
            is_init_permute: F::from_bool(i == 0),
            is_permute: F::from_bool(i != 0),
        };
        let sponge_datum = poseidon2
            .sponge_data
            .get(i as usize)
            .expect("unroll_count not consistent with number of permutations");
        unroll.push(Poseidon2Sponge {
            clk: F::from_canonical_u64(row.state.clk),
            ops,
            input_addr: F::from_canonical_u32(input_addr),
            output_addr: F::from_canonical_u32(output_addr),
            input_len: F::from_canonical_u32(input_len),
            preimage: sponge_datum.preimage,
            output: sponge_datum.output,
            gen_output: sponge_datum.gen_output,
            input_addr_padded: F::from_canonical_u32(input_addr_padded),
        });
        input_addr_padded += u32::try_from(MozakPoseidon2::DATA_PADDING).expect("Should succeed");
        input_addr += rate_size;
        input_len -= rate_size;
    }

    unroll
}

#[must_use]
pub fn generate_poseidon2_sponge_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<Poseidon2Sponge<F>> {
    let trace = pad_poseidon2_sponge_trace(
        filter(step_rows)
            .map(|s| unroll_sponge_data(s))
            .collect_vec()
            .into_iter()
            .flatten()
            .collect::<Vec<Poseidon2Sponge<F>>>(),
    );
    log::trace!("Poseidon2 Sponge trace {:#?}", trace);
    trace
}

#[cfg(test)]
mod test {
    use mozak_runner::poseidon2::MozakPoseidon2;
    use plonky2::field::types::Field;
    use plonky2::hash::hashing::PlonkyPermutation;
    use plonky2::hash::poseidon2::Poseidon2Permutation;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::generation::MIN_TRACE_LENGTH;
    use crate::poseidon2_sponge::columns::Poseidon2Sponge;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn generate_poseidon2_sponge_trace() {
        let data = "😇 Mozak is knowledge arguments based technology".to_string();
        let data_len_in_bytes = MozakPoseidon2::do_padding(data.as_bytes()).len();
        let input_start_addr = 1024;
        let output_start_addr = 2048;
        let (_program, record) = create_poseidon2_test(&[Poseidon2Test {
            data,
            input_start_addr,
            output_start_addr,
        }]);

        let step_rows = record.executed;
        let trace = super::generate_poseidon2_sponge_trace(&step_rows);

        let rate_size = Poseidon2Permutation::<F>::RATE;
        let sponge_count =
            (data_len_in_bytes / MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT) / rate_size;
        for (i, value) in trace.iter().enumerate().take(sponge_count) {
            assert_eq!(
                value.input_addr,
                F::from_canonical_usize(
                    usize::try_from(input_start_addr).expect("can't fail") + i * rate_size
                )
            );
        }
        assert_eq!(
            trace.len(),
            sponge_count.next_power_of_two().max(MIN_TRACE_LENGTH)
        );
    }
    #[test]
    fn generate_poseidon2_sponge_trace_with_dummy() {
        let step_rows = vec![];
        let trace: Vec<Poseidon2Sponge<F>> = super::generate_poseidon2_sponge_trace(&step_rows);
        assert_eq!(trace.len(), MIN_TRACE_LENGTH);
    }
}
