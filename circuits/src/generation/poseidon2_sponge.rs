use itertools::Itertools;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::poseidon2_sponge::columns::{Ops, Poseidon2Sponge};

fn pad_poseidon2_sponge_trace<F: RichField>(
    mut trace: Vec<Poseidon2Sponge<F>>,
) -> Vec<Poseidon2Sponge<F>> {
    trace.resize(trace.len().next_power_of_two(), Poseidon2Sponge {
        ..Default::default()
    });
    trace
}

pub fn filter<F: RichField>(step_rows: &[Row<F>]) -> impl Iterator<Item = &Row<F>> {
    step_rows.iter().filter(|row| row.aux.poseidon2.is_some())
}

fn unroll_sponge_data<F: RichField>(row: &Row<F>) -> Vec<Poseidon2Sponge<F>> {
    let poseidon2 = row.aux.poseidon2.clone().expect("please pass filtered row");
    let mut unroll = vec![];
    let rate_bits = 8;
    assert!(poseidon2.len % rate_bits == 0);
    let unroll_count = poseidon2.len / rate_bits;
    assert!(poseidon2.sponge_data.len() == unroll_count as usize);

    for i in 0..unroll_count {
        let ops: Ops<F> = if i == 0 {
            // init_permute row
            Ops {
                is_init_permute: F::ONE,
                is_permute: F::ZERO,
            }
        } else {
            Ops {
                is_init_permute: F::ZERO,
                is_permute: F::ONE,
            }
        };
        let sponge_datum = poseidon2
            .sponge_data
            .get(i as usize)
            .expect("unroll_count not consistant with number of permutations");
        unroll.push(Poseidon2Sponge {
            clk: F::from_canonical_u64(row.state.clk),
            ops,
            addr: F::from_canonical_u32(poseidon2.addr),
            out_addr: F::from_canonical_u32(poseidon2.output_addr),
            start_index: F::from_canonical_u32(poseidon2.len - (i * rate_bits)),
            preimage: sponge_datum.preimage,
            output: sponge_datum.output,
            is_exe: F::ONE,
            gen_output: sponge_datum.gen_output,
        });
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
    log::trace!("Poseidon2 Sponge trace {:?}", trace);
    trace
}
