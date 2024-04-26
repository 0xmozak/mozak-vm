#![allow(clippy::too_many_lines)]

use std::collections::HashMap;

use anyhow::ensure;
use itertools::Itertools;
use log::Level::Debug;
use log::{debug, info, log_enabled};
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::extension::Extendable;
use plonky2::field::packable::Packable;
use plonky2::field::polynomial::{PolynomialCoeffs, PolynomialValues};
use plonky2::field::types::Field;
use plonky2::fri::batch_oracle::BatchFriOracle;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::fri::proof::FriProof;
use plonky2::fri::structure::{FriBatchInfo, FriInstanceInfo, FriOracleInfo};
use plonky2::hash::hash_types::RichField;
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::GenericConfig;
use plonky2::timed;
use plonky2::util::log2_strict;
use plonky2::util::timing::TimingTree;
#[allow(clippy::wildcard_imports)]
use plonky2_maybe_rayon::*;
use starky::config::StarkConfig;
use starky::stark::{LookupConfig, Stark};

use super::mozak_stark::{MozakStark, TableKind, TableKindArray, TableKindSetBuilder};
use super::proof::{BatchProof, StarkOpeningSet, StarkProof};
use crate::cross_table_lookup::ctl_utils::debug_ctl;
use crate::cross_table_lookup::{cross_table_lookup_data, CtlData};
use crate::generation::{debug_traces, generate_traces};
use crate::public_sub_table::public_sub_table_data_and_values;
use crate::stark::mozak_stark::{all_kind, all_starks, PublicInputs};
use crate::stark::permutation::challenge::GrandProductChallengeTrait;
use crate::stark::poly::compute_quotient_polys;
use crate::stark::prover::prove_single_table;

pub(crate) fn batch_fri_instances<F: RichField + Extendable<D>, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    public_table_kinds: &[TableKind],
    degree_bits: &TableKindArray<usize>,
    sorted_degree_bits: &[usize],
    zeta: F::Extension,
    config: &StarkConfig,
    num_ctl_zs_per_table: &TableKindArray<usize>,
) -> Vec<FriInstanceInfo<F, D>> {
    let fri_instances = all_starks!(mozak_stark, |stark, kind| if !public_table_kinds
        .contains(&kind)
    {
        Some({
            let g = F::primitive_root_of_unity(degree_bits[kind]);

            stark.fri_instance(
                zeta,
                g,
                0,
                vec![],
                config,
                Some(&LookupConfig {
                    degree_bits: degree_bits[kind],
                    num_zs: num_ctl_zs_per_table[kind],
                }),
            )
        })
    } else {
        None
    });

    let mut degree_log_map: HashMap<usize, Vec<TableKind>> = HashMap::new();
    all_kind!(|kind| {
        degree_log_map
            .entry(degree_bits[kind])
            .or_insert(Vec::new())
            .push(kind);
    });

    let fri_instance_groups = sorted_degree_bits
        .iter()
        .map(|degree_log| {
            degree_log_map[degree_log]
                .iter()
                .filter_map(|kind| fri_instances[*kind].as_ref())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut polynomial_index_start = [0, 0, 0];
    let res = fri_instance_groups
        .iter()
        .map(|ins| merge_fri_instances(ins, &mut polynomial_index_start))
        .collect::<Vec<_>>();
    res
}

pub(crate) fn merge_fri_instances<F: RichField + Extendable<D>, const D: usize>(
    instances: &[&FriInstanceInfo<F, D>],
    polynomial_index_start: &mut [usize; 3],
) -> FriInstanceInfo<F, D> {
    assert!(!instances.is_empty());
    let base_instance = &instances[0];
    assert_eq!(base_instance.oracles.len(), 3);
    assert_eq!(base_instance.batches.len(), 3);

    let mut res = FriInstanceInfo {
        oracles: Vec::with_capacity(3),
        batches: Vec::with_capacity(3),
    };

    for i in 0..3 {
        res.oracles.push(FriOracleInfo {
            num_polys: 0,
            blinding: base_instance.oracles[i].blinding,
        });
        res.batches.push(FriBatchInfo {
            point: base_instance.batches[i].point,
            polynomials: vec![],
        });
    }

    for ins in instances {
        assert_eq!(ins.oracles.len(), 3);
        assert_eq!(ins.batches.len(), 3);

        for i in 0..3 {
            assert_eq!(res.oracles[i].blinding, ins.oracles[i].blinding);
            res.oracles[i].num_polys += ins.oracles[i].num_polys;

            assert_eq!(res.batches[i].point, ins.batches[i].point);
            for poly in ins.batches[i].polynomials.iter().cloned() {
                let mut poly = poly;
                poly.polynomial_index += polynomial_index_start[poly.oracle_index];
                // assert!(
                //     poly.polynomial_index < res.oracles[poly.oracle_index].num_polys,
                //     "{}, {}, ",
                //     poly.polynomial_index,
                //     res.oracles[poly.oracle_index].num_polys
                // );
                res.batches[i].polynomials.push(poly);
            }
        }

        for i in 0..3 {
            polynomial_index_start[i] += ins.oracles[i].num_polys;
        }
    }

    res
}

pub fn batch_prove<F, C, const D: usize>(
    program: &Program,
    record: &ExecutionRecord<F>,
    mozak_stark: &MozakStark<F, D>,
    public_table_kinds: &[TableKind],
    config: &StarkConfig,
    public_inputs: PublicInputs<F>,
    timing: &mut TimingTree,
) -> anyhow::Result<BatchProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    debug!("Starting Prove");
    let traces_poly_values = generate_traces(program, record);
    if mozak_stark.debug || std::env::var("MOZAK_STARK_DEBUG").is_ok() {
        debug_traces(&traces_poly_values, mozak_stark, &public_inputs);
        debug_ctl(&traces_poly_values, mozak_stark);
    }
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;

    let degree_bits = all_kind!(|kind| log2_strict(traces_poly_values[kind][0].len()));

    let batch_traces_poly_values = all_kind!(|kind| if public_table_kinds.contains(&kind) {
        None
    } else {
        Some(&traces_poly_values[kind])
    });

    let mut batch_trace_polys: Vec<_> = batch_traces_poly_values
        .iter()
        .filter_map(|t| *t)
        .flat_map(|v| v.clone())
        .collect();
    batch_trace_polys.sort_by(|a, b| b.len().cmp(&a.len()));
    let bacth_trace_polys_len = batch_trace_polys.len();

    let batch_trace_commitments: BatchFriOracle<F, C, D> = timed!(
        timing,
        "Compute trace commitments for batch tables",
        BatchFriOracle::from_values(
            batch_trace_polys,
            rate_bits,
            false,
            cap_height,
            timing,
            &vec![None; bacth_trace_polys_len],
        )
    );

    let trace_commitments = timed!(
        timing,
        "Compute trace commitments for each table",
        traces_poly_values
            .clone()
            .with_kind()
            .map(|(trace, table)| {
                timed!(
                    timing,
                    &format!("compute trace commitment for {table:?}"),
                    PolynomialBatch::<F, C, D>::from_values(
                        trace.clone(),
                        rate_bits,
                        false,
                        cap_height,
                        timing,
                        None,
                    )
                )
            })
    );

    let trace_caps = trace_commitments
        .each_ref()
        .map(|c| c.merkle_tree.cap.clone());
    // Add trace commitments to the challenger entropy pool.
    let mut challenger = Challenger::<F, C::Hasher>::new();
    all_kind!(|kind| {
        if public_table_kinds.contains(&kind) {
            challenger.observe_cap(&trace_caps[kind]);
        }
    });
    let fmt_trace_cap = batch_trace_commitments.field_merkle_tree.cap.clone();
    challenger.observe_cap(&fmt_trace_cap);

    let ctl_challenges = challenger.get_grand_product_challenge_set(config.num_challenges);
    let ctl_data_per_table = timed!(
        timing,
        "Compute CTL data for each table",
        cross_table_lookup_data::<F, D>(
            &traces_poly_values,
            &mozak_stark.cross_table_lookups,
            &ctl_challenges
        )
    );

    let (public_sub_table_data_per_table, public_sub_table_values) =
        public_sub_table_data_and_values::<F, D>(
            &traces_poly_values,
            &mozak_stark.public_sub_tables,
            &ctl_challenges,
        );

    let (proofs, batch_stark_proof) = batch_prove_with_commitments(
        mozak_stark,
        config,
        &public_table_kinds,
        &public_inputs,
        &degree_bits,
        &traces_poly_values,
        &trace_commitments,
        &batch_trace_commitments,
        &ctl_data_per_table,
        &public_sub_table_data_per_table,
        // todo: remove clone()
        &mut challenger.clone(),
        timing,
    )?;

    let program_rom_trace_cap = trace_caps[TableKind::Program].clone();
    let elf_memory_init_trace_cap = trace_caps[TableKind::ElfMemoryInit].clone();
    if log_enabled!(Debug) {
        timing.print();
    }
    Ok(BatchProof {
        degree_bits,
        proofs,
        program_rom_trace_cap,
        elf_memory_init_trace_cap,
        public_inputs,
        public_sub_table_values,
        batch_stark_proof,
    })
}

/// Given the traces generated from [`generate_traces`] along with their
/// commitments, prove a [`MozakStark`].
///
/// # Errors
/// Errors if proving fails.
#[allow(clippy::too_many_arguments)]
pub fn batch_prove_with_commitments<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    public_table_kinds: &[TableKind],
    public_inputs: &PublicInputs<F>,
    degree_bits: &TableKindArray<usize>,
    traces_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    trace_commitments: &TableKindArray<PolynomialBatch<F, C, D>>,
    batch_trace_commitments: &BatchFriOracle<F, C, D>,
    ctl_data_per_table: &TableKindArray<CtlData<F>>,
    public_sub_data_per_table: &TableKindArray<CtlData<F>>,
    challenger: &mut Challenger<F, C::Hasher>,
    timing: &mut TimingTree,
) -> anyhow::Result<(TableKindArray<StarkProof<F, C, D>>, StarkProof<F, C, D>)>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;

    let cpu_stark = [public_inputs.entry_point];
    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_stark: &cpu_stark,
        ..Default::default()
    }
    .build();

    let separate_proofs = all_starks!(mozak_stark, |stark, kind| if public_table_kinds
        .contains(&kind)
    {
        Some(prove_single_table(
            stark,
            config,
            &traces_poly_values[kind],
            &trace_commitments[kind],
            public_inputs[kind],
            &ctl_data_per_table[kind],
            &public_sub_data_per_table[kind],
            challenger,
            timing,
        )?)
    } else {
        None
    });

    let batch_ctl_z_polys = all_kind!(|kind| {
        if !public_table_kinds.contains(&kind) {
            Some({
                let fri_params = config.fri_params(degree_bits[kind]);
                assert!(
                    fri_params.total_arities() <= degree_bits[kind] + rate_bits - cap_height,
                    "FRI total reduction arity is too large.",
                );

                let z_poly_public_sub_table = public_sub_data_per_table[kind].z_polys();

                let z_polys = vec![ctl_data_per_table[kind].z_polys(), z_poly_public_sub_table]
                    .into_iter()
                    .flatten()
                    .collect_vec();

                assert!(!z_polys.is_empty());

                info!(
                    "ctl_data_per_table len {}",
                    ctl_data_per_table[kind].zs_columns.len()
                );
                info!("z_poly len {}", z_polys.len());

                z_polys
            })
        } else {
            None
        }
    });

    // TODO: can we remove duplicates in the ctl polynomials?
    let mut batch_ctl_zs_polys: Vec<_> = batch_ctl_z_polys
        .iter()
        .filter_map(|t| t.as_ref())
        .flat_map(|v| v.iter().cloned())
        .collect();
    batch_ctl_zs_polys.sort_by(|a, b| b.len().cmp(&a.len()));
    let batch_ctl_zs_polys_len = batch_ctl_zs_polys.len();

    let batch_ctl_zs_commitments: BatchFriOracle<F, C, D> = timed!(
        timing,
        "compute batch Zs commitment",
        BatchFriOracle::from_values(
            batch_ctl_zs_polys,
            rate_bits,
            false,
            config.fri_config.cap_height,
            timing,
            &vec![None; batch_ctl_zs_polys_len],
        )
    );

    let ctl_zs_commitments = all_starks!(mozak_stark, |stark, kind| timed!(
        timing,
        format!("{stark}: compute Zs commitment").as_str(),
        if let Some(poly) = &batch_ctl_z_polys[kind] {
            Some(PolynomialBatch::<F, C, D>::from_values(
                poly.clone(),
                rate_bits,
                false,
                config.fri_config.cap_height,
                timing,
                None,
            ))
        } else {
            None
        }
    ));

    let ctl_zs_cap = batch_ctl_zs_commitments.field_merkle_tree.cap.clone();
    challenger.observe_cap(&ctl_zs_cap);

    let alphas = challenger.get_n_challenges(config.num_challenges);

    // TODO: we should be able to compute `quotient_polys` from
    // `batch_trace_commitments` and `batch_ctl_zs_commitments`.
    let quotient_chunks = all_starks!(mozak_stark, |stark, kind| {
        if let Some(ctl_zs_commitment) = ctl_zs_commitments[kind].as_ref() {
            let degree = 1 << degree_bits[kind];
            let quotient_polys = timed!(
                timing,
                format!("{stark}: compute quotient polynomial").as_str(),
                compute_quotient_polys::<F, <F as Packable>::Packing, C, _, D>(
                    stark,
                    &trace_commitments[kind],
                    &ctl_zs_commitment,
                    public_inputs[kind],
                    &ctl_data_per_table[kind],
                    &public_sub_data_per_table[kind],
                    &alphas,
                    degree_bits[kind],
                    config,
                )
            );
            assert!(!quotient_polys.is_empty());

            let quotient_chunks: Vec<PolynomialCoeffs<F>> = timed!(
                timing,
                format!("{stark}: split quotient polynomial").as_str(),
                quotient_polys
                    .into_par_iter()
                    .flat_map(|mut quotient_poly| {
                        quotient_poly
                    .trim_to_len(degree * stark.quotient_degree_factor())
                    .expect(
                        "Quotient has failed, the vanishing polynomial is not divisible by Z_H",
                    );
                        // Split quotient into degree-n chunks.
                        quotient_poly.chunks(degree)
                    })
                    .collect()
            );
            Some(quotient_chunks)
        } else {
            None
        }
    });

    let mut batch_quotient_chunks: Vec<_> = quotient_chunks
        .iter()
        .filter_map(|t| t.as_ref())
        .flat_map(|v| v.iter().cloned())
        .collect();
    batch_quotient_chunks.sort_by(|a, b| b.len().cmp(&a.len()));
    let batch_quotient_chunks_len = batch_quotient_chunks.len();

    let quotient_commitments = all_starks!(mozak_stark, |stark, kind| timed!(
        timing,
        format!("{stark}: compute quotient commitment").as_str(),
        if let Some(poly) = &quotient_chunks[kind] {
            Some(PolynomialBatch::<F, C, D>::from_coeffs(
                poly.clone(),
                rate_bits,
                false,
                config.fri_config.cap_height,
                timing,
                None,
            ))
        } else {
            None
        }
    ));

    let batch_quotient_commitments: BatchFriOracle<F, C, D> = timed!(
        timing,
        "compute batch Zs commitment",
        BatchFriOracle::from_coeffs(
            batch_quotient_chunks,
            rate_bits,
            false,
            config.fri_config.cap_height,
            timing,
            &vec![None; batch_quotient_chunks_len],
        )
    );

    let quotient_polys_cap = batch_quotient_commitments.field_merkle_tree.cap.clone();
    challenger.observe_cap(&quotient_polys_cap);

    let zeta = challenger.get_extension_challenge::<D>();

    // TODO: compute `openings` from `batch_trace_commitments` and
    // `batch_ctl_zs_commitments`.
    let batch_openings = all_starks!(mozak_stark, |_stark, kind| if let Some(ctl_zs_commitment) =
        ctl_zs_commitments[kind].as_ref()
    {
        if let Some(quotient_commitment) = quotient_commitments[kind].as_ref() {
            // To avoid leaking witness data, we want to ensure that our opening locations,
            // `zeta` and `g * zeta`, are not in our subgroup `H`. It suffices to check
            // `zeta` only, since `(g * zeta)^n = zeta^n`, where `n` is the order of
            // `g`.
            let g = F::primitive_root_of_unity(degree_bits[kind]);
            ensure!(
                zeta.exp_power_of_2(degree_bits[kind]) != F::Extension::ONE,
                "Opening point is in the subgroup."
            );
            let openings = StarkOpeningSet::new(
                zeta,
                g,
                &trace_commitments[kind],
                &ctl_zs_commitment,
                &quotient_commitment,
                degree_bits[kind],
            );

            challenger.observe_openings(&openings.to_fri_openings());
            Some(openings)
        } else {
            None
        }
    } else {
        None
    });

    // Merge FRI instances by its polynomial degree
    let mut sorted_degree_bits: Vec<usize> =
        all_kind!(|kind| (!public_table_kinds.contains(&kind)).then_some(degree_bits[kind]))
            .iter()
            .filter_map(|d| *d)
            .collect_vec();
    sorted_degree_bits.sort();
    sorted_degree_bits.reverse();
    sorted_degree_bits.dedup();

    let num_ctl_zs_per_table =
        all_kind!(|kind| ctl_data_per_table[kind].len() + public_sub_data_per_table[kind].len());

    let batch_fri_instances = batch_fri_instances(
        mozak_stark,
        public_table_kinds,
        degree_bits,
        &sorted_degree_bits,
        zeta,
        config,
        &num_ctl_zs_per_table,
    );

    let initial_merkle_trees = vec![
        batch_trace_commitments,
        &batch_ctl_zs_commitments,
        &batch_quotient_commitments,
    ];

    for i in 0..3 {
        assert_eq!(
            initial_merkle_trees[i].polynomials.len(),
            batch_fri_instances
                .iter()
                .map(|ins| ins.oracles[i].num_polys)
                .collect::<Vec<usize>>()
                .iter()
                .sum::<usize>(),
            "batch index: {i}"
        );
    }

    let mut fri_params = config.fri_params(sorted_degree_bits[0]);
    fri_params.reduction_arity_bits =
        batch_reduction_arity_bits(sorted_degree_bits.clone(), rate_bits, cap_height);
    let opening_proof = timed!(
        timing,
        format!("compute batch opening proofs").as_str(),
        BatchFriOracle::prove_openings(
            &sorted_degree_bits,
            &batch_fri_instances,
            &initial_merkle_trees,
            challenger,
            &fri_params,
            timing,
        )
    );

    let empty_fri_proof = FriProof {
        commit_phase_merkle_caps: vec![],
        query_round_proofs: vec![],
        final_poly: PolynomialCoeffs { coeffs: vec![] },
        pow_witness: F::ZERO,
    };
    let empty_opening_set = StarkOpeningSet {
        local_values: vec![],
        next_values: vec![],
        ctl_zs: vec![],
        ctl_zs_next: vec![],
        ctl_zs_last: vec![],
        quotient_polys: vec![],
    };
    Ok((
        all_kind!(|kind| {
            if public_table_kinds.contains(&kind) {
                <Option<StarkProof<F, C, D>> as Clone>::clone(&separate_proofs[kind])
                    .expect("No Proof")
            } else {
                StarkProof {
                    trace_cap: MerkleCap::default(),
                    ctl_zs_cap: MerkleCap::default(),
                    quotient_polys_cap: MerkleCap::default(),
                    openings: <Option<StarkOpeningSet<F, D>> as Clone>::clone(
                        &batch_openings[kind],
                    )
                    .expect("No Openings"),
                    opening_proof: empty_fri_proof.clone(),
                }
            }
        }),
        StarkProof {
            trace_cap: batch_trace_commitments.field_merkle_tree.cap.clone(),
            ctl_zs_cap,
            quotient_polys_cap,
            openings: empty_opening_set,
            opening_proof,
        },
    ))
}

// TODO: find a better place for this function
fn batch_reduction_arity_bits(
    degree_bits: Vec<usize>,
    rate_bits: usize,
    cap_height: usize,
) -> Vec<usize> {
    let mut result = Vec::new();
    let arity_bits = 3;
    let mut cur_index = 0;
    let mut cur_degree_bits = degree_bits[cur_index];
    while cur_degree_bits + rate_bits >= cap_height + arity_bits {
        let mut cur_arity_bits = arity_bits;
        let target_degree_bits = cur_degree_bits - arity_bits;
        if cur_index < degree_bits.len() - 1 && target_degree_bits < degree_bits[cur_index + 1] {
            cur_arity_bits = cur_degree_bits - degree_bits[cur_index + 1];
            cur_index += 1;
        }
        result.push(cur_arity_bits);
        assert!(cur_degree_bits >= cur_arity_bits);
        cur_degree_bits -= cur_arity_bits;
    }
    result
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::timing::TimingTree;

    use crate::stark::batch_prover::batch_prove;
    use crate::stark::batch_verifier::batch_verify_proof;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs, TableKind};
    use crate::stark::proof::BatchProof;
    use crate::test_utils::fast_test_config;
    use crate::utils::from_u32;

    #[test]
    fn batch_prove_add() {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, 3), (7, 4)],
        );
        let config = fast_test_config();

        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

        let stark: MozakStark<F, D> = MozakStark::default();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        // We cannot batch prove these tables because trace caps are needed as public
        // inputs for the following tables.
        let public_table_kinds = vec![TableKind::Program, TableKind::ElfMemoryInit];

        let all_proof: BatchProof<F, C, D> = batch_prove(
            &program,
            &record,
            &stark,
            &public_table_kinds,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        )
        .unwrap();
        batch_verify_proof(&stark, &public_table_kinds, all_proof, &config).unwrap();
    }
}