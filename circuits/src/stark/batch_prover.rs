#![allow(clippy::too_many_lines)]

use std::collections::HashMap;

use anyhow::{ensure, Result};
use itertools::{chain, Itertools};
use log::Level::Debug;
use log::{debug, log_enabled};
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

#[derive(Debug)]
pub struct BatchFriOracleIndices {
    poly_count: TableKindArray<usize>,
    // start index in BatchFriOracle's field merkle tree leaves
    fmt_start_indices: TableKindArray<Option<usize>>,
    // start index in BatchFriOracle's polynomial vector
    poly_start_indices: TableKindArray<Option<usize>>,
    // degree bits (leaf layer) index in BatchFriOrable's field merkle tree
    degree_bits_indices: TableKindArray<Option<usize>>,
}

#[allow(unused_assignments)]
impl BatchFriOracleIndices {
    fn new(
        public_table_kinds: &[TableKind],
        poly_count: TableKindArray<usize>,
        degree_bits: &TableKindArray<usize>,
    ) -> Self {
        let sorted_degree_bits = sort_degree_bits(public_table_kinds, degree_bits);

        let mut poly_start_indices =
            all_kind!(|kind| (!public_table_kinds.contains(&kind)).then_some(0));
        let mut fmt_start_indices =
            all_kind!(|kind| (!public_table_kinds.contains(&kind)).then_some(0));
        let mut poly_start_index = 0;
        for deg in &sorted_degree_bits {
            let mut fmt_start_index = 0;
            all_kind!(|kind| {
                if !public_table_kinds.contains(&kind) && degree_bits[kind] == *deg {
                    fmt_start_indices[kind] = Some(fmt_start_index);
                    poly_start_indices[kind] = Some(poly_start_index);
                    fmt_start_index += poly_count[kind];
                    poly_start_index += poly_count[kind];
                }
            });
        }

        let degree_bits_index_map: HashMap<usize, usize> = sorted_degree_bits
            .into_iter()
            .enumerate()
            .map(|(index, value)| (value, index))
            .collect();
        let degree_bits_indices = all_kind!(|kind| (!public_table_kinds.contains(&kind))
            .then_some(degree_bits_index_map[&degree_bits[kind]]));

        BatchFriOracleIndices {
            poly_count,
            fmt_start_indices,
            poly_start_indices,
            degree_bits_indices,
        }
    }
}

pub(crate) fn sort_degree_bits(
    public_table_kinds: &[TableKind],
    degree_bits: &TableKindArray<usize>,
) -> Vec<usize> {
    let mut sorted_degree_bits: Vec<usize> =
        all_kind!(|kind| (!public_table_kinds.contains(&kind)).then_some(degree_bits[kind]))
            .iter()
            .filter_map(|d| *d)
            .collect_vec();
    sorted_degree_bits.sort_unstable();
    sorted_degree_bits.reverse();
    sorted_degree_bits.dedup();
    sorted_degree_bits
}

pub(crate) fn batch_fri_instances<F: RichField + Extendable<D>, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    public_table_kinds: &[TableKind],
    degree_bits: &TableKindArray<usize>,
    sorted_degree_bits: &[usize],
    zeta: F::Extension,
    config: &StarkConfig,
    num_ctl_zs_per_table: &TableKindArray<usize>,
) -> Vec<FriInstanceInfo<F, D>> {
    let fri_instances = all_starks!(
        mozak_stark,
        |stark, kind| if public_table_kinds.contains(&kind) {
            None
        } else {
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
        }
    );

    let mut degree_bits_map: HashMap<usize, Vec<TableKind>> = HashMap::new();
    all_kind!(|kind| {
        degree_bits_map
            .entry(degree_bits[kind])
            .or_default()
            .push(kind);
    });

    let fri_instance_groups = sorted_degree_bits
        .iter()
        .map(|d| {
            degree_bits_map[d]
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

// Merge FRI instances by its polynomial degree
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
            for poly in ins.batches[i].polynomials.iter().copied() {
                let mut poly = poly;
                poly.polynomial_index += polynomial_index_start[poly.oracle_index];
                res.batches[i].polynomials.push(poly);
            }
        }

        for (i, item) in polynomial_index_start.iter_mut().enumerate().take(3) {
            *item += ins.oracles[i].num_polys;
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
) -> Result<BatchProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    debug!("Starting Prove");
    let traces_poly_values = generate_traces(program, record, timing);
    if mozak_stark.debug || std::env::var("MOZAK_STARK_DEBUG").is_ok() {
        debug_traces(&traces_poly_values, mozak_stark, &public_inputs);
        debug_ctl(&traces_poly_values, mozak_stark);
    }
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;
    let degree_bits = all_kind!(|kind| log2_strict(traces_poly_values[kind][0].len()));
    let traces_poly_count = all_kind!(|kind| traces_poly_values[kind].len());
    let trace_indices =
        BatchFriOracleIndices::new(public_table_kinds, traces_poly_count, &degree_bits);

    let batch_traces_poly_values = all_kind!(|kind| if public_table_kinds.contains(&kind) {
        None
    } else {
        Some(&traces_poly_values[kind])
    });

    let mut batch_trace_polys: Vec<_> = batch_traces_poly_values
        .iter()
        .filter_map(|t| *t)
        .flat_map(std::clone::Clone::clone)
        .collect();
    batch_trace_polys.sort_by_key(|p| std::cmp::Reverse(p.len()));

    // This commitment is for all tables but public tables, in form of Field Merkle
    // Tree (1st oracle)
    let batch_trace_polys_len = batch_trace_polys.len();
    let batch_trace_commitments: BatchFriOracle<F, C, D> = timed!(
        timing,
        "Compute trace commitments for batch tables",
        BatchFriOracle::from_values(
            batch_trace_polys,
            rate_bits,
            false,
            cap_height,
            timing,
            &vec![None; batch_trace_polys_len],
        )
    );

    // This commitment is for public tables, and would have separate oracle.
    let trace_commitments = timed!(
        timing,
        "Compute trace commitments for public tables",
        traces_poly_values
            .clone()
            .with_kind()
            .map(|(trace, table)| {
                public_table_kinds.contains(&table).then(|| {
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
            })
    );

    let trace_caps = all_kind!(|kind| trace_commitments[kind]
        .as_ref()
        .map(|c| c.merkle_tree.cap.clone()));

    // Add trace commitments to the challenger entropy pool.
    let mut challenger = Challenger::<F, C::Hasher>::new();
    all_kind!(|kind| {
        if let Some(c) = trace_caps[kind].clone() {
            challenger.observe_cap(&c);
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
        public_table_kinds,
        &public_inputs,
        &degree_bits,
        &traces_poly_values,
        &trace_commitments,
        &trace_indices,
        &batch_trace_commitments,
        &ctl_data_per_table,
        &public_sub_table_data_per_table,
        &mut challenger,
        timing,
    )?;

    let program_rom_trace_cap = trace_caps[TableKind::Program].clone().unwrap();
    let elf_memory_init_trace_cap = trace_caps[TableKind::ElfMemoryInit].clone().unwrap();
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
#[allow(clippy::type_complexity)]
pub fn batch_prove_with_commitments<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    public_table_kinds: &[TableKind],
    public_inputs: &PublicInputs<F>,
    degree_bits: &TableKindArray<usize>,
    traces_poly_values: &TableKindArray<Vec<PolynomialValues<F>>>,
    trace_commitments: &TableKindArray<Option<PolynomialBatch<F, C, D>>>,
    trace_indices: &BatchFriOracleIndices,
    batch_trace_commitments: &BatchFriOracle<F, C, D>,
    ctl_data_per_table: &TableKindArray<CtlData<F>>,
    public_sub_data_per_table: &TableKindArray<CtlData<F>>,
    challenger: &mut Challenger<F, C::Hasher>,
    timing: &mut TimingTree,
) -> Result<(TableKindArray<StarkProof<F, C, D>>, StarkProof<F, C, D>)>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;

    // TODO(Matthias): Unify everything in this function with the non-batch version.
    let cpu_skeleton_stark = [public_inputs.entry_point];
    let public_inputs = TableKindSetBuilder::<&[_]> {
        cpu_skeleton_stark: &cpu_skeleton_stark,
        ..Default::default()
    }
    .build();

    // Computes separate proofs for each public table.
    let separate_proofs = all_starks!(mozak_stark, |stark, kind| if let Some(trace_commitment) =
        &trace_commitments[kind]
    {
        Some(prove_single_table(
            stark,
            config,
            &traces_poly_values[kind],
            trace_commitment,
            public_inputs[kind],
            &ctl_data_per_table[kind],
            &public_sub_data_per_table[kind],
            challenger,
            timing,
        )?)
    } else {
        None
    });

    // Computing ctl zs polynomials for all but those for public tables
    let mut ctl_zs_poly_count = all_kind!(|_kind| 0);
    let all_ctl_z_polys = all_kind!(|kind| {
        if public_table_kinds.contains(&kind) {
            None
        } else {
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

                ctl_zs_poly_count[kind] = z_polys.len();

                z_polys
            })
        }
    });

    let ctl_zs_indices =
        BatchFriOracleIndices::new(public_table_kinds, ctl_zs_poly_count, degree_bits);

    // TODO: can we remove duplicates in the ctl polynomials?
    let mut batch_ctl_z_polys: Vec<_> = all_ctl_z_polys
        .iter()
        .filter_map(|t| t.as_ref())
        .flat_map(|v| v.iter().cloned())
        .collect();
    batch_ctl_z_polys.sort_by_key(|b| std::cmp::Reverse(b.len()));
    let batch_ctl_zs_polys_len = batch_ctl_z_polys.len();

    // Commitment to all ctl_zs polynomials, except for those of public tables.
    // Field Merkle tree is used as oracle here, same as we did for batched traces.
    // (2nd FMT Oracle)
    let batch_ctl_zs_commitments: BatchFriOracle<F, C, D> = timed!(
        timing,
        "compute batch Zs commitment",
        BatchFriOracle::from_values(
            batch_ctl_z_polys,
            rate_bits,
            false,
            config.fri_config.cap_height,
            timing,
            &vec![None; batch_ctl_zs_polys_len],
        )
    );

    let ctl_zs_cap = batch_ctl_zs_commitments.field_merkle_tree.cap.clone();
    challenger.observe_cap(&ctl_zs_cap);

    let alphas = challenger.get_n_challenges(config.num_challenges);
    let sorted_degree_bits = sort_degree_bits(public_table_kinds, degree_bits);

    let mut quotient_poly_count = all_kind!(|_kind| 0);
    let quotient_chunks = all_starks!(mozak_stark, |stark, kind| {
        if public_table_kinds.contains(&kind) {
            None
        } else {
            let degree = 1 << degree_bits[kind];

            let degree_bits_index = trace_indices.degree_bits_indices[kind].unwrap();
            let trace_slice_start = trace_indices.fmt_start_indices[kind].unwrap();
            let trace_slice_len = trace_indices.poly_count[kind];
            let get_trace_values_packed = |i_start, step| -> Vec<<F as Packable>::Packing> {
                batch_trace_commitments.get_lde_values_packed(
                    degree_bits_index,
                    i_start,
                    step,
                    trace_slice_start,
                    trace_slice_len,
                )
            };

            let ctl_zs_slice_start = ctl_zs_indices.fmt_start_indices[kind].unwrap();
            let ctl_zs_slice_len = ctl_zs_indices.poly_count[kind];
            let get_ctl_zs_values_packed = |i_start, step| -> Vec<<F as Packable>::Packing> {
                batch_ctl_zs_commitments.get_lde_values_packed(
                    degree_bits_index,
                    i_start,
                    step,
                    ctl_zs_slice_start,
                    ctl_zs_slice_len,
                )
            };

            let quotient_polys = timed!(
                timing,
                format!("{stark}: compute quotient polynomial").as_str(),
                compute_quotient_polys::<F, <F as Packable>::Packing, C, _, D>(
                    stark,
                    &get_trace_values_packed,
                    &get_ctl_zs_values_packed,
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
            quotient_poly_count[kind] = quotient_chunks.len();
            Some(quotient_chunks)
        }
    });

    let quotient_indices =
        BatchFriOracleIndices::new(public_table_kinds, quotient_poly_count, degree_bits);

    let mut batch_quotient_chunks: Vec<_> = quotient_chunks
        .iter()
        .filter_map(|t| t.as_ref())
        .flat_map(|v| v.iter().cloned())
        .collect();
    batch_quotient_chunks.sort_by_key(|b| std::cmp::Reverse(b.len()));
    let batch_quotient_chunks_len = batch_quotient_chunks.len();

    // Commitment to quotient polynomials for all except those of public tables.
    // Stored as Field merkle tree (3rd and final FMT oracle)
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

    // Sets up batched fri instance for all tables but the public tables.
    let batch_openings = all_starks!(mozak_stark, |_stark, kind| if public_table_kinds
        .contains(&kind)
    {
        None
    } else {
        // To avoid leaking witness data, we want to ensure that our opening locations,
        // `zeta` and `g * zeta`, are not in our subgroup `H`. It suffices to check
        // `zeta` only, since `(g * zeta)^n = zeta^n`, where `n` is the order of
        // `g`.
        let g = F::primitive_root_of_unity(degree_bits[kind]);
        ensure!(
            zeta.exp_power_of_2(degree_bits[kind]) != F::Extension::ONE,
            "Opening point is in the subgroup."
        );

        let openings = StarkOpeningSet::batch_new(
            zeta,
            g,
            [
                trace_indices.poly_start_indices[kind].unwrap(),
                ctl_zs_indices.poly_start_indices[kind].unwrap(),
                quotient_indices.poly_start_indices[kind].unwrap(),
            ],
            [
                trace_indices.poly_count[kind],
                ctl_zs_indices.poly_count[kind],
                quotient_indices.poly_count[kind],
            ],
            batch_trace_commitments,
            &batch_ctl_zs_commitments,
            &batch_quotient_commitments,
            degree_bits[kind],
        );

        challenger.observe_openings(&openings.to_fri_openings());
        Some(openings)
    });

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

    for (i, tree) in initial_merkle_trees.iter().enumerate() {
        assert_eq!(
            tree.polynomials.len(),
            batch_fri_instances
                .iter()
                .map(|ins| ins.oracles[i].num_polys)
                .sum::<usize>(),
            "batch index: {i}"
        );
    }

    let mut fri_params = config.fri_params(sorted_degree_bits[0]);
    fri_params.reduction_arity_bits =
        batch_reduction_arity_bits(&sorted_degree_bits.clone(), rate_bits, cap_height);
    let opening_proof = timed!(
        timing,
        "compute batch opening proofs",
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
pub(crate) fn batch_reduction_arity_bits(
    degree_bits: &[usize],
    rate_bits: usize,
    cap_height: usize,
) -> Vec<usize> {
    let default_arity_bits = 3;
    let final_poly_bits = 5;
    let lowest_degree_bits = degree_bits.last().unwrap();
    assert!(lowest_degree_bits + rate_bits >= cap_height);
    // First, let's figure out our intermediate degree bits.
    let intermediate_degree_bits =
        degree_bits
            .iter()
            .tuple_windows()
            .flat_map(|(&degree_bit, &next_degree_bit)| {
                (next_degree_bit + 1..=degree_bit)
                    .rev()
                    .step_by(default_arity_bits)
            });
    // Next, deal with the last part.
    let last_degree_bits =
        (lowest_degree_bits + rate_bits).min(cap_height.max(final_poly_bits)) - rate_bits;
    let final_degree_bits = (last_degree_bits..=*lowest_degree_bits)
        .rev()
        .step_by(default_arity_bits);
    // Finally, the reduction arity bits are just the differences:
    chain!(intermediate_degree_bits, final_degree_bits)
        .tuple_windows()
        .map(|(degree_bit, next_degree_bit)| degree_bit - next_degree_bit)
        .collect()
}

#[cfg(test)]
mod tests {
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::timing::TimingTree;

    use crate::stark::batch_prover::{batch_prove, batch_reduction_arity_bits};
    use crate::stark::batch_verifier::batch_verify_proof;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs, TableKind};
    use crate::stark::proof::BatchProof;
    use crate::test_utils::fast_test_config;
    use crate::utils::from_u32;

    #[test]
    fn reduction_arity_bits_in_batch_proving() {
        let degree_bits = vec![15, 8, 6, 5, 3];
        let rate_bits = 2;
        let cap_height = 0;
        let expected_res = vec![3, 3, 1, 2, 1, 2];
        assert_eq!(
            expected_res,
            batch_reduction_arity_bits(&degree_bits, rate_bits, cap_height)
        );

        let rate_bits = 1;
        let cap_height = 4;
        let expected_res = vec![3, 3, 1, 2, 1, 2];
        assert_eq!(
            expected_res,
            batch_reduction_arity_bits(&degree_bits, rate_bits, cap_height)
        );
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn bad_reduction_arity_bits_in_batch_proving() {
        let degree_bits = vec![8, 6, 5, 3];
        let rate_bits = 2;
        let cap_height = 6;
        batch_reduction_arity_bits(&degree_bits, rate_bits, cap_height);
    }

    #[test]
    fn batch_prove_add() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;

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
