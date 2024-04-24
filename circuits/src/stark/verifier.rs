use std::borrow::Borrow;

use anyhow::{ensure, Result};
use log::debug;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;
use starky::proof::MultiProof;
use starky::stark::Stark;

use super::mozak_stark::{all_starks, MozakStark, TableKind, TableKindSetBuilder};
use super::proof::AllProof;
use crate::stark::proof::AllProofChallenges;

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

    let num_lookup_columns = all_starks!(mozak_stark, |stark, _kind| stark
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
    // TODO(Matthias): we still need to make sure that all the challenges are
    // correct, via our own observing etc.
    all_starks!(mozak_stark, |stark, kind| {
        starky::verifier::verify_stark_proof_with_challenges(
            stark,
            &all_proof.proofs.each_ref()[kind].proof,
            &stark_challenges[kind],
            Some(&ctl_vars_per_table[kind as usize]),
            public_inputs[kind],
            config,
        )
        .unwrap_or_else(|e| panic!("Failed to verify stark proof for {kind:?}: {e}"));
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
