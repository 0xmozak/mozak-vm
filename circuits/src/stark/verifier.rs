use std::borrow::Borrow;

use anyhow::{ensure, Result};
use itertools::Itertools;
use log::debug;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::types::Field;
use plonky2::fri::verifier::verify_fri_proof;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::plonk_common::reduce_with_powers;
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::{LookupConfig, Stark};

use super::mozak_stark::{all_starks, MozakStark, TableKind, TableKindSetBuilder};
use super::proof::AllProof;
use crate::cross_table_lookup::{verify_cross_table_lookups_and_public_sub_tables, CtlCheckVars};
use crate::public_sub_table::reduce_public_sub_tables_values;
use crate::stark::poly::eval_vanishing_poly;
use crate::stark::proof::{AllProofChallenges, StarkOpeningSet, StarkProof, StarkProofChallenges};

#[allow(clippy::too_many_lines)]
pub fn verify_proof<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    all_proof: AllProof<F, C, D>,
    config: &StarkConfig,
) -> Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    debug!("Starting Verify");

    let AllProofChallenges {
        stark_challenges,
        ctl_challenges,
    } = all_proof.get_challenges(config);

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

    let reduced_public_sub_tables_values =
        reduce_public_sub_tables_values(&all_proof.public_sub_table_values, &ctl_challenges);

    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_stark: all_proof.public_inputs.borrow(),
        ..Default::default()
    }
    .build();
    all_starks!(mozak_stark, |stark, kind| {
        verify_stark_proof_with_challenges(
            stark,
            &all_proof.proofs[kind],
            &stark_challenges[kind],
            public_inputs[kind],
            &ctl_vars_per_table[kind],
            config,
        )?;
    });
    verify_cross_table_lookups_and_public_sub_tables::<F, D>(
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &reduced_public_sub_tables_values,
        &all_proof.all_ctl_zs_last(),
        config,
    )?;
    Ok(())
}

pub(crate) fn verify_quotient_polynomials<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    stark: &S,
    degree_bits: usize,
    proof: &StarkProof<F, C, D>,
    challenges: &StarkProofChallenges<F, D>,
    public_inputs: &[F],
    ctl_vars: &[CtlCheckVars<F, F::Extension, F::Extension, D>],
) -> Result<()>
where
{
    let StarkOpeningSet {
        local_values,
        next_values,
        ctl_zs: _,
        ctl_zs_next: _,
        ctl_zs_last: _,
        quotient_polys,
    } = &proof.openings;

    let vars = S::EvaluationFrame::from_values(
        local_values,
        next_values,
        &public_inputs
            .iter()
            .map(|pi| F::Extension::from_basefield(*pi))
            .collect_vec(),
    );

    let (l_0, l_last) = eval_l_0_and_l_last(degree_bits, challenges.stark_zeta);
    let last = F::primitive_root_of_unity(degree_bits).inverse();
    let z_last = challenges.stark_zeta - last.into();
    let mut consumer = ConstraintConsumer::<F::Extension>::new(
        challenges
            .stark_alphas
            .iter()
            .map(|&alpha| F::Extension::from_basefield(alpha))
            .collect::<Vec<_>>(),
        z_last,
        l_0,
        l_last,
    );
    eval_vanishing_poly::<F, F::Extension, F::Extension, S, D, D>(
        stark,
        &vars,
        ctl_vars,
        &mut consumer,
    );
    let vanishing_polys_zeta = consumer.accumulators();

    // Check each polynomial identity, of the form `vanishing(x) = Z_H(x)
    // quotient(x)`, at zeta.
    let zeta_pow_deg = challenges.stark_zeta.exp_power_of_2(degree_bits);
    let z_h_zeta = zeta_pow_deg - F::Extension::ONE;
    // `quotient_polys_zeta` holds `num_challenges * quotient_degree_factor`
    // evaluations. Each chunk of `quotient_degree_factor` holds the evaluations
    // of `t_0(zeta),...,t_{quotient_degree_factor-1}(zeta)` where the "real"
    // quotient polynomial is `t(X) = t_0(X) + t_1(X)*X^n + t_2(X)*X^{2n} + ...`.
    // So to reconstruct `t(zeta)` we can compute `reduce_with_powers(chunk,
    // zeta^n)` for each `quotient_degree_factor`-sized chunk of the original
    // evaluations.
    for (i, chunk) in quotient_polys
        .chunks(stark.quotient_degree_factor())
        .enumerate()
    {
        ensure!(
            vanishing_polys_zeta[i] == z_h_zeta * reduce_with_powers(chunk, zeta_pow_deg),
            "Mismatch between evaluation and opening of quotient polynomial"
        );
    }

    // Make sure that we do not use Starky's lookups.
    assert!(!stark.requires_ctls());
    assert!(!stark.uses_lookups());

    Ok(())
}

pub(crate) fn verify_stark_proof_with_challenges<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    stark: &S,
    proof: &StarkProof<F, C, D>,
    challenges: &StarkProofChallenges<F, D>,
    public_inputs: &[F],
    ctl_vars: &[CtlCheckVars<F, F::Extension, F::Extension, D>],
    config: &StarkConfig,
) -> Result<()>
where
{
    validate_proof_shape(stark, proof, config, ctl_vars.len())?;
    let degree_bits = proof.recover_degree_bits(config);
    verify_quotient_polynomials(
        stark,
        degree_bits,
        proof,
        challenges,
        public_inputs,
        ctl_vars,
    )?;

    let ctl_zs_last = &proof.openings.ctl_zs_last;
    let merkle_caps = vec![
        proof.trace_cap.clone(),
        proof.ctl_zs_cap.clone(),
        proof.quotient_polys_cap.clone(),
    ];

    verify_fri_proof::<F, C, D>(
        &stark.fri_instance(
            challenges.stark_zeta,
            F::primitive_root_of_unity(degree_bits),
            0,
            vec![],
            config,
            Some(&LookupConfig {
                degree_bits,
                num_zs: ctl_zs_last.len(),
            }),
        ),
        &proof.openings.to_fri_openings(),
        &challenges.fri_challenges,
        &merkle_caps,
        &proof.opening_proof,
        &config.fri_params(degree_bits),
    )?;

    Ok(())
}

fn validate_proof_shape<F, C, S, const D: usize>(
    stark: &S,
    proof: &StarkProof<F, C, D>,
    config: &StarkConfig,
    num_ctl_zs: usize,
) -> anyhow::Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>, {
    let StarkProof {
        trace_cap,
        ctl_zs_cap,
        quotient_polys_cap,
        openings,
        // The shape of the opening proof will be checked in the FRI verifier (see
        // validate_fri_proof_shape), so we ignore it here.
        opening_proof: _,
    } = proof;

    let StarkOpeningSet {
        local_values,
        next_values,
        ctl_zs,
        ctl_zs_next,
        ctl_zs_last,
        quotient_polys,
    } = openings;

    let degree_bits = proof.recover_degree_bits(config);
    let fri_params = config.fri_params(degree_bits);
    let cap_height = fri_params.config.cap_height;

    ensure!(trace_cap.height() == cap_height);
    ensure!(ctl_zs_cap.height() == cap_height);
    ensure!(quotient_polys_cap.height() == cap_height);

    ensure!(local_values.len() == S::COLUMNS);
    ensure!(next_values.len() == S::COLUMNS);
    ensure!(ctl_zs.len() == num_ctl_zs);
    ensure!(ctl_zs_next.len() == num_ctl_zs);
    ensure!(ctl_zs_last.len() == num_ctl_zs);
    ensure!(quotient_polys.len() == stark.num_quotient_polys(config));

    Ok(())
}

/// Evaluate the Lagrange polynomials `L_0` and `L_(n-1)` at a point `x`.
/// `L_0(x) = (x^n - 1)/(n * (x - 1))`
/// `L_(n-1)(x) = (x^n - 1)/(n * (g * x - 1))`, with `g` the first element of
/// the subgroup.
fn eval_l_0_and_l_last<F: Field>(log_n: usize, x: F) -> (F, F) {
    let n = F::from_canonical_usize(1 << log_n);
    let g = F::primitive_root_of_unity(log_n);
    let z_x = x.exp_power_of_2(log_n) - F::ONE;
    let invs = F::batch_multiplicative_inverse(&[n * (x - F::ONE), n * (g * x - F::ONE)]);

    (z_x * invs[0], z_x * invs[1])
}

#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::polynomial::PolynomialValues;
    use plonky2::field::types::Sample;

    use crate::stark::verifier::eval_l_0_and_l_last;

    #[test]
    fn test_eval_l_0_and_l_last() {
        type F = GoldilocksField;
        let log_n = 5;
        let n = 1 << log_n;

        let x = F::rand(); // challenge point
        let expected_l_first_x = PolynomialValues::selector(n, 0).ifft().eval(x);
        let expected_l_last_x = PolynomialValues::selector(n, n - 1).ifft().eval(x);

        let (l_first_x, l_last_x) = eval_l_0_and_l_last(log_n, x);
        assert_eq!(l_first_x, expected_l_first_x);
        assert_eq!(l_last_x, expected_l_last_x);
    }
}
