use std::borrow::Borrow;

use anyhow::ensure;
use itertools::Itertools;
use log::debug;
use plonky2::field::extension::Extendable;
use plonky2::fri::batch_verifier::verify_batch_fri_proof;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;

use super::mozak_stark::{all_kind, all_starks, MozakStark, TableKind, TableKindSetBuilder};
use crate::cross_table_lookup::{verify_cross_table_lookups_and_public_sub_tables, CtlCheckVars};
use crate::public_sub_table::reduce_public_sub_tables_values;
use crate::stark::batch_prover::{batch_fri_instances, batch_reduction_arity_bits};
use crate::stark::permutation::challenge::GrandProductChallengeTrait;
use crate::stark::proof::BatchProof;
use crate::stark::verifier::{verify_quotient_polynomials, verify_stark_proof_with_challenges};

#[allow(clippy::too_many_lines)]
pub fn batch_verify_proof<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    public_table_kinds: &[TableKind],
    all_proof: BatchProof<F, C, D>,
    config: &StarkConfig,
) -> anyhow::Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    debug!("Starting Verify");
    let mut challenger = Challenger::<F, C::Hasher>::new();

    for kind in public_table_kinds {
        challenger.observe_cap(&all_proof.proofs[*kind].trace_cap);
    }
    challenger.observe_cap(&all_proof.batch_stark_proof.trace_cap);

    // TODO: Observe public values.

    let ctl_challenges = challenger.get_grand_product_challenge_set(config.num_challenges);

    let stark_challenges = all_kind!(|kind| {
        if public_table_kinds.contains(&kind) {
            challenger.compact();
            Some(all_proof.proofs[kind].get_challenges(&mut challenger, config))
        } else {
            None
        }
    });

    let batch_stark_challenges = all_proof
        .batch_stark_proof
        .get_challenges(&mut challenger, config);

    ensure!(
        all_proof.proofs[TableKind::Program].trace_cap == all_proof.program_rom_trace_cap,
        "Mismatch between Program ROM trace caps"
    );

    ensure!(
        all_proof.proofs[TableKind::ElfMemoryInit].trace_cap == all_proof.elf_memory_init_trace_cap,
        "Mismatch between ElfMemoryInit trace caps"
    );

    let ctl_vars_per_table = CtlCheckVars::from_proofs(
        &all_proof.proofs,
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &ctl_challenges,
    );

    let degree_bits = all_proof.degree_bits;

    let reduced_public_sub_tables_values =
        reduce_public_sub_tables_values(&all_proof.public_sub_table_values, &ctl_challenges);

    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_stark: all_proof.public_inputs.borrow(),
        ..Default::default()
    }
    .build();
    all_starks!(mozak_stark, |stark, kind| {
        if public_table_kinds.contains(&kind) {
            if let Some(challenges) = &stark_challenges[kind] {
                verify_stark_proof_with_challenges(
                    stark,
                    &all_proof.proofs[kind],
                    &challenges,
                    public_inputs[kind],
                    &ctl_vars_per_table[kind],
                    config,
                )?;
            } else {
                ensure!(false);
            }
        } else {
            verify_quotient_polynomials(
                stark,
                degree_bits[kind],
                &all_proof.proofs[kind],
                &batch_stark_challenges,
                public_inputs[kind],
                &ctl_vars_per_table[kind],
            )?;
        }
    });

    let num_ctl_zs_per_table = all_kind!(|kind| all_proof.proofs[kind].openings.ctl_zs_last.len());
    let all_ctl_zs_last = all_proof.proofs.clone().map(|p| p.openings.ctl_zs_last);
    verify_cross_table_lookups_and_public_sub_tables::<F, D>(
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &reduced_public_sub_tables_values,
        &all_ctl_zs_last,
        config,
    )?;

    let mut sorted_degree_bits: Vec<usize> =
        all_kind!(|kind| (!public_table_kinds.contains(&kind)).then_some(degree_bits[kind]))
            .iter()
            .filter_map(|d| *d)
            .collect_vec();
    sorted_degree_bits.sort();
    sorted_degree_bits.reverse();
    sorted_degree_bits.dedup();

    let fri_instances = batch_fri_instances(
        mozak_stark,
        public_table_kinds,
        &degree_bits,
        &sorted_degree_bits,
        batch_stark_challenges.stark_zeta,
        config,
        &num_ctl_zs_per_table,
    );
    let stark_proof = all_proof.batch_stark_proof;
    let proof = stark_proof.opening_proof;
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;
    let mut fri_params = config.fri_params(sorted_degree_bits[0]);
    fri_params.reduction_arity_bits =
        batch_reduction_arity_bits(sorted_degree_bits.clone(), rate_bits, cap_height);
    let fri_challenges = challenger.fri_challenges::<C, D>(
        &proof.commit_phase_merkle_caps,
        &proof.final_poly,
        proof.pow_witness,
        sorted_degree_bits[0],
        &fri_params.config,
    );
    let init_merkle_caps = [
        stark_proof.trace_cap,
        stark_proof.ctl_zs_cap,
        stark_proof.quotient_polys_cap,
    ];

    let mut fri_openings = Vec::with_capacity(sorted_degree_bits.len());
    for (i, d) in sorted_degree_bits.iter().enumerate() {
        all_kind!(|kind| if degree_bits[kind] == *d {
            if fri_openings.len() == i {
                fri_openings.push(all_proof.proofs[kind].openings.to_fri_openings());
            } else {
                // TODO: this is for debugging purposes only
                fri_openings[i]
                    .batches
                    .iter()
                    .zip_eq(
                        all_proof.proofs[kind]
                            .openings
                            .to_fri_openings()
                            .batches
                            .iter(),
                    )
                    .all(|(b0, b1)| {
                        b0.values
                            .iter()
                            .zip_eq(b1.values.iter())
                            .all(|(v0, v1)| v0 == v1)
                    });
            }
        });
    }

    sorted_degree_bits = sorted_degree_bits
        .iter()
        .map(|d| d + fri_params.config.rate_bits)
        .collect_vec();
    verify_batch_fri_proof::<F, C, D>(
        &sorted_degree_bits,
        &fri_instances,
        &fri_openings,
        &fri_challenges,
        &init_merkle_caps,
        &proof,
        &fri_params,
    )?;

    Ok(())
}
