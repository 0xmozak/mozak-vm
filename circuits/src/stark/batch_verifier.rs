use std::borrow::Borrow;

use anyhow::{ensure, Result};
use log::debug;
use plonky2::field::extension::Extendable;
use plonky2::fri::batch_verifier::verify_batch_fri_proof;
use plonky2::fri::proof::FriProof;
use plonky2::fri::structure::{FriOpeningBatch, FriOpenings};
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use starky::config::StarkConfig;

use super::mozak_stark::{
    all_kind, all_starks, MozakStark, TableKind, TableKindArray, TableKindSetBuilder,
};
use crate::cross_table_lookup::{verify_cross_table_lookups_and_public_sub_tables, CtlCheckVars};
use crate::public_sub_table::reduce_public_sub_tables_values;
use crate::stark::batch_prover::{
    batch_fri_instances, batch_reduction_arity_bits, sort_degree_bits,
};
use crate::stark::permutation::challenge::GrandProductChallengeTrait;
use crate::stark::proof::{BatchProof, StarkProof, StarkProofChallenges};
use crate::stark::prover::get_program_id;
use crate::stark::verifier::{verify_quotient_polynomials, verify_stark_proof_with_challenges};

#[allow(clippy::too_many_lines)]
pub fn batch_verify_proof<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    public_table_kinds: &[TableKind],
    all_proof: BatchProof<F, C, D>,
    config: &StarkConfig,
    degree_bits: &TableKindArray<usize>,
) -> Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
    debug!("Starting Batch Verify");

    let sorted_degree_bits = sort_degree_bits(public_table_kinds, degree_bits);

    let mut challenger = Challenger::<F, C::Hasher>::new();

    for kind in public_table_kinds {
        challenger.observe_cap(&all_proof.proofs[*kind].trace_cap);
    }
    challenger.observe_cap(&all_proof.batch_stark_proof.trace_cap);

    // TODO: Observe public values.

    let ctl_challenges = challenger.get_grand_product_challenge_set(config.num_challenges);

    // Get challenges for public STARKs.
    let stark_challenges = all_kind!(|kind| {
        if public_table_kinds.contains(&kind) {
            challenger.compact();
            Some(all_proof.proofs[kind].get_challenges(&mut challenger, config))
        } else {
            None
        }
    });

    // Get challenges for the batch STARK.
    let batch_stark_challenges = {
        let StarkProof {
            ctl_zs_cap,
            quotient_polys_cap,
            opening_proof:
                FriProof {
                    commit_phase_merkle_caps,
                    final_poly,
                    pow_witness,
                    ..
                },
            ..
        } = &all_proof.batch_stark_proof;

        let num_challenges = config.num_challenges;

        challenger.observe_cap(ctl_zs_cap);

        let stark_alphas = challenger.get_n_challenges(num_challenges);

        challenger.observe_cap(quotient_polys_cap);
        let stark_zeta = challenger.get_extension_challenge::<D>();

        all_kind!(|kind| if !public_table_kinds.contains(&kind) {
            challenger.observe_openings(&all_proof.proofs[kind].openings.to_fri_openings());
        });

        StarkProofChallenges {
            stark_alphas,
            stark_zeta,
            fri_challenges: challenger.fri_challenges::<C, D>(
                commit_phase_merkle_caps,
                final_poly,
                *pow_witness,
                sorted_degree_bits[0],
                &config.fri_config,
            ),
        }
    };

    let ctl_vars_per_table = CtlCheckVars::from_proofs(
        &all_proof.proofs,
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &ctl_challenges,
    );

    let reduced_public_sub_tables_values =
        reduce_public_sub_tables_values(&all_proof.public_sub_table_values, &ctl_challenges);

    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_skeleton_stark: all_proof.public_inputs.borrow(),
        ..Default::default()
    }
    .build();

    let program_id = get_program_id::<F, C, D>(
        all_proof.public_inputs.entry_point,
        &all_proof.proofs[TableKind::Program].trace_cap,
        &all_proof.proofs[TableKind::ElfMemoryInit].trace_cap,
    );
    ensure!(program_id == all_proof.program_id);

    all_starks!(mozak_stark, |stark, kind| {
        if public_table_kinds.contains(&kind) {
            if let Some(challenges) = &stark_challenges[kind] {
                // Verifying public tables proof, including individual FRI proof
                verify_stark_proof_with_challenges(
                    stark,
                    &all_proof.proofs[kind],
                    challenges,
                    public_inputs[kind],
                    &ctl_vars_per_table[kind],
                    config,
                )?;
            } else {
                ensure!(false);
            }
        } else {
            // Verifying quotient polynomials of the batched stark proof (for all starks but
            // public starks). Batched FRI proof for the openings to be done later.
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

    let fri_instances = batch_fri_instances(
        mozak_stark,
        public_table_kinds,
        degree_bits,
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
        batch_reduction_arity_bits(&sorted_degree_bits.clone(), rate_bits, cap_height);
    let init_merkle_caps = [
        stark_proof.trace_cap,
        stark_proof.ctl_zs_cap,
        stark_proof.quotient_polys_cap,
    ];

    let batch_count = 3;
    let empty_fri_opening = FriOpenings {
        batches: (0..batch_count)
            .map(|_| FriOpeningBatch { values: vec![] })
            .collect(),
    };
    let mut fri_openings = vec![empty_fri_opening; sorted_degree_bits.len()];

    for (i, d) in sorted_degree_bits.iter().enumerate() {
        all_kind!(
            |kind| if degree_bits[kind] == *d && !public_table_kinds.contains(&kind) {
                let openings = all_proof.proofs[kind].openings.to_fri_openings();
                assert!(openings.batches.len() == batch_count);
                for j in 0..batch_count {
                    fri_openings[i].batches[j]
                        .values
                        .extend(openings.batches[j].values.clone());
                }
            }
        );
    }

    verify_batch_fri_proof::<F, C, D>(
        &sorted_degree_bits,
        &fri_instances,
        &fri_openings,
        &batch_stark_challenges.fri_challenges,
        &init_merkle_caps,
        &proof,
        &fri_params,
    )?;

    Ok(())
}
