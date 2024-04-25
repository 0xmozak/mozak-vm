use std::borrow::Borrow;

use anyhow::ensure;
use log::debug;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;

use super::mozak_stark::{all_kind, all_starks, MozakStark, TableKind, TableKindSetBuilder};
use crate::cross_table_lookup::{verify_cross_table_lookups_and_public_sub_tables, CtlCheckVars};
use crate::public_sub_table::reduce_public_sub_tables_values;
use crate::stark::permutation::challenge::GrandProductChallengeTrait;
use crate::stark::proof::BatchProof;
use crate::stark::verifier::verify_stark_proof_with_challenges;

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

    let stark_challenges = all_kind!(|kind| if public_table_kinds.contains(&kind) {
        challenger.compact();
        Some(all_proof.proofs[kind].get_challenges(&mut challenger, config))
    } else {
        None
    });

    let _batch_stark_challenges = all_proof
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
                    challenges, // Use the unwrapped challenges here
                    public_inputs[kind],
                    &ctl_vars_per_table[kind],
                    config,
                )?;
            } else {
                ensure!(false);
            }
        }
    });
    let all_ctl_zs_last = all_proof.proofs.map(|p| p.openings.ctl_zs_last);
    verify_cross_table_lookups_and_public_sub_tables::<F, D>(
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &reduced_public_sub_tables_values,
        &all_ctl_zs_last,
        config,
    )?;
    // verify_batch_stark_proof_with_challenges(
    //
    // );
    Ok(())
}
