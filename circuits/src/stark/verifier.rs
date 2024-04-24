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
use starky::proof::{MultiProof, StarkProofWithMetadata};
use starky::stark::{LookupConfig, Stark};

use super::mozak_stark::{all_starks, MozakStark, TableKind, TableKindSetBuilder};
use super::proof::AllProof;
use crate::cross_table_lookup::CtlCheckVars;
use crate::stark::poly::eval_vanishing_poly;
use crate::stark::proof::{AllProofChallenges, StarkOpeningSet, StarkProof, StarkProofChallenges};

#[allow(clippy::too_many_lines)]
pub fn verify_proof<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    all_proof: &AllProof<F, C, D>,
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
        all_proof.proofs[TableKind::Program].proof.trace_cap == all_proof.program_rom_trace_cap,
        "Mismatch between Program ROM trace caps"
    );

    ensure!(
        all_proof.proofs[TableKind::ElfMemoryInit].proof.trace_cap
            == all_proof.elf_memory_init_trace_cap,
        "Mismatch between ElfMemoryInit trace caps"
    );

    let num_lookup_columns = all_starks!(mozak_stark, |stark, kind| stark
        .num_lookup_helper_columns(config))
    .0;
    let multi_proof = MultiProof {
        // TODO(Matthias): this is also a bit silly.  But proofs are small-ish, and we only clone
        // once.
        stark_proofs: all_proof.proofs.0.clone(),
        // TODO(Matthias): we only use the multi_proof once, and that usage doesn't actualyl read
        // the ctl-challenges. That's probably a sloppiness in plonky2.
        ctl_challenges: ctl_challenges.clone(),
    };
    let ctl_vars_per_table = starky::cross_table_lookup::get_ctl_vars_from_proofs(
        // &all_proof.proofs,
        &multi_proof,
        &mozak_stark.cross_table_lookups,
        &ctl_challenges,
        &num_lookup_columns,
        // TODO(Matthias): perhaps don't hardcode this?
        3,
    );

    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_stark: all_proof.public_inputs.borrow(),
        ..Default::default()
    }
    .build();
    all_starks!(mozak_stark, |stark, kind| {
        starky::verifier::verify_stark_proof_with_challenges(
            stark,
            &all_proof.proofs.each_ref()[kind].proof,
            &stark_challenges[kind],
            Some(&ctl_vars_per_table[kind as usize]),
            public_inputs[kind],
            config,
        )?;
    });
    starky::cross_table_lookup::verify_cross_table_lookups(
        &mozak_stark.cross_table_lookups,
        all_proof
            .proofs
            .each_ref()
            .map(|p| p.proof.openings.ctl_zs_first.clone().unwrap())
            .0,
        // TODO(Matthias): zk_evm uses this to simulate our pub sub mechanism in a different way.
        None,
        config,
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
