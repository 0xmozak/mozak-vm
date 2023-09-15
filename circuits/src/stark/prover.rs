#![allow(clippy::too_many_lines)]

use anyhow::{ensure, Result};
use itertools::Itertools;
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::extension::Extendable;
use plonky2::field::packable::Packable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::{GenericConfig, Hasher};
use plonky2::timed;
use plonky2::util::log2_strict;
use plonky2::util::timing::TimingTree;
use plonky2_maybe_rayon::{MaybeIntoParIter, ParallelIterator};
use starky::config::StarkConfig;
use starky::stark::{LookupConfig, Stark};

use super::mozak_stark::{MozakStark, TableKind, NUM_TABLES};
use super::proof::{AllProof, StarkOpeningSet, StarkProof};
use crate::bitshift::stark::BitshiftStark;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::ctl_utils::debug_ctl;
use crate::cross_table_lookup::{cross_table_lookup_data, CtlData};
use crate::generation::{debug_traces, generate_traces};
use crate::memory::stark::MemoryStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::stark::mozak_stark::PublicInputs;
use crate::stark::permutation::challenge::{GrandProductChallengeSet, GrandProductChallengeTrait};
use crate::stark::permutation::compute_permutation_z_polys;
use crate::stark::poly::compute_quotient_polys;
use crate::xor::stark::XorStark;

pub fn prove<F, C, const D: usize>(
    program: &Program,
    record: &ExecutionRecord,
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    public_inputs: PublicInputs<F>,
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); RangeCheckStark::<F, D>::PUBLIC_INPUTS]:,
    [(); XorStark::<F, D>::COLUMNS]:,
    [(); BitshiftStark::<F, D>::COLUMNS]:,
    [(); ProgramStark::<F, D>::COLUMNS]:,
    [(); MemoryStark::<F, D>::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:, {
    let traces_poly_values = generate_traces(program, record);
    // if mozak_stark.debug || std::env::var("MOZAK_STARK_DEBUG").is_ok() {
    debug_traces(&traces_poly_values, mozak_stark, &public_inputs);
    debug_ctl(&traces_poly_values, mozak_stark);
    // }
    prove_with_traces(
        mozak_stark,
        config,
        public_inputs,
        &traces_poly_values,
        timing,
    )
}

/// Given the traces generated from [`generate_traces`], prove a [`MozakStark`].
///
/// # Errors
/// Errors if proving fails.
pub fn prove_with_traces<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    public_inputs: PublicInputs<F>,
    traces_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); RangeCheckStark::<F, D>::PUBLIC_INPUTS]:,
    [(); XorStark::<F, D>::COLUMNS]:,
    [(); BitshiftStark::<F, D>::COLUMNS]:,
    [(); ProgramStark::<F, D>::COLUMNS]:,
    [(); MemoryStark::<F, D>::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:, {
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;

    let trace_commitments = timed!(
        timing,
        "Compute trace commitments for each table",
        traces_poly_values
            .iter()
            .zip_eq(TableKind::all())
            .map(|(trace, table)| {
                timed!(
                    timing,
                    &format!("compute trace commitment for {table:?}"),
                    PolynomialBatch::<F, C, D>::from_values(
                        // TODO: Cloning this isn't great; consider having `from_values` accept a
                        // reference,
                        // or having `compute_permutation_z_polys` read trace values from the
                        // `PolynomialBatch`.
                        trace.clone(),
                        rate_bits,
                        false,
                        cap_height,
                        timing,
                        None,
                    )
                )
            })
            .collect::<Vec<_>>()
    );

    let trace_caps = trace_commitments
        .iter()
        .map(|c| c.merkle_tree.cap.clone())
        .collect::<Vec<_>>();
    // Add trace commitments to the challenger entropy pool.
    let mut challenger = Challenger::<F, C::Hasher>::new();
    for cap in &trace_caps {
        challenger.observe_cap(cap);
    }

    let ctl_challenges = challenger.get_grand_product_challenge_set(config.num_challenges);
    let ctl_data_per_table = timed!(
        timing,
        "Compute CTL data for each table",
        cross_table_lookup_data::<F, D>(
            traces_poly_values,
            &mozak_stark.cross_table_lookups,
            &ctl_challenges
        )
    );
    let stark_proofs = timed!(
        timing,
        "compute all proofs given commitments",
        prove_with_commitments(
            mozak_stark,
            config,
            &public_inputs,
            traces_poly_values,
            &trace_commitments,
            &ctl_data_per_table,
            &mut challenger,
            timing
        )?
    );

    let program_rom_trace_cap = trace_caps[TableKind::Program as usize].clone();
    Ok(AllProof {
        stark_proofs,
        program_rom_trace_cap,
        public_inputs,
    })
}

/// Compute proof for a single STARK table, with lookup data.
///
/// # Errors
/// Errors if FRI parameters are wrongly configured, or if
/// there are no z polys, or if our
/// opening points are in our subgroup `H`,
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_single_table<F, C, S, const D: usize>(
    stark: &S,
    config: &StarkConfig,
    trace_poly_values: &[PolynomialValues<F>],
    trace_commitment: &PolynomialBatch<F, C, D>,
    public_inputs: [F; S::PUBLIC_INPUTS],
    ctl_data: &CtlData<F>,
    challenger: &mut Challenger<F, C::Hasher>,
    timing: &mut TimingTree,
) -> Result<StarkProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    [(); C::Hasher::HASH_SIZE]:,
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:, {
    let degree = trace_poly_values[0].len();
    let degree_bits = log2_strict(degree);
    let fri_params = config.fri_params(degree_bits);
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;
    assert!(
        fri_params.total_arities() <= degree_bits + rate_bits - cap_height,
        "FRI total reduction arity is too large.",
    );

    challenger.compact();

    // Permutation arguments.
    let permutation_challenges: Vec<GrandProductChallengeSet<F>> = challenger
        .get_n_grand_product_challenge_sets(config.num_challenges, stark.permutation_batch_size());
    let mut permutation_zs = timed!(
        timing,
        "compute permutation Z(x) polys",
        compute_permutation_z_polys::<F, S, D>(
            stark,
            config,
            trace_poly_values,
            &permutation_challenges
        )
    );
    let num_permutation_zs = permutation_zs.len();

    let z_polys = {
        permutation_zs.extend(ctl_data.z_polys());
        permutation_zs
    };
    // TODO(Matthias): make the code work with empty z_polys, too.
    assert!(!z_polys.is_empty(), "No CTL?");

    let permutation_ctl_zs_commitment = timed!(
        timing,
        "compute Zs commitment",
        PolynomialBatch::from_values(
            z_polys,
            rate_bits,
            false,
            config.fri_config.cap_height,
            timing,
            None,
        )
    );

    let permutation_ctl_zs_cap = permutation_ctl_zs_commitment.merkle_tree.cap.clone();
    challenger.observe_cap(&permutation_ctl_zs_cap);

    let alphas = challenger.get_n_challenges(config.num_challenges);
    let quotient_polys = timed!(
        timing,
        "compute quotient polys",
        compute_quotient_polys::<F, <F as Packable>::Packing, C, S, D>(
            stark,
            trace_commitment,
            &permutation_ctl_zs_commitment,
            &permutation_challenges,
            public_inputs,
            ctl_data,
            &alphas,
            degree_bits,
            num_permutation_zs,
            config,
        )
    );

    let all_quotient_chunks = timed!(
        timing,
        "split quotient polys",
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
    let quotient_commitment = timed!(
        timing,
        "compute quotient commitment",
        PolynomialBatch::from_coeffs(
            all_quotient_chunks,
            rate_bits,
            false,
            config.fri_config.cap_height,
            timing,
            None,
        )
    );
    let quotient_polys_cap = quotient_commitment.merkle_tree.cap.clone();
    challenger.observe_cap(&quotient_polys_cap);

    let zeta = challenger.get_extension_challenge::<D>();
    // To avoid leaking witness data, we want to ensure that our opening locations,
    // `zeta` and `g * zeta`, are not in our subgroup `H`. It suffices to check
    // `zeta` only, since `(g * zeta)^n = zeta^n`, where `n` is the order of
    // `g`.
    let g = F::primitive_root_of_unity(degree_bits);
    ensure!(
        zeta.exp_power_of_2(degree_bits) != F::Extension::ONE,
        "Opening point is in the subgroup."
    );

    let openings = StarkOpeningSet::new(
        zeta,
        g,
        trace_commitment,
        &permutation_ctl_zs_commitment,
        &quotient_commitment,
        degree_bits,
        stark.num_permutation_batches(config),
    );

    challenger.observe_openings(&openings.to_fri_openings());

    let initial_merkle_trees = vec![
        trace_commitment,
        &permutation_ctl_zs_commitment,
        &quotient_commitment,
    ];

    let opening_proof = timed!(
        timing,
        "compute openings proof",
        PolynomialBatch::prove_openings(
            &stark.fri_instance(
                zeta,
                g,
                config,
                Some(&LookupConfig {
                    degree_bits,
                    num_zs: ctl_data.len()
                })
            ),
            &initial_merkle_trees,
            challenger,
            &fri_params,
            timing,
        )
    );

    Ok(StarkProof {
        trace_cap: trace_commitment.merkle_tree.cap.clone(),
        permutation_ctl_zs_cap,
        quotient_polys_cap,
        openings,
        opening_proof,
    })
}

/// Given the traces generated from [`generate_traces`] along with their
/// commitments, prove a [`MozakStark`].
///
/// # Errors
/// Errors if proving fails.
#[allow(clippy::too_many_arguments)]
pub fn prove_with_commitments<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    public_inputs: &PublicInputs<F>,
    traces_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    trace_commitments: &[PolynomialBatch<F, C, D>],
    ctl_data_per_table: &[CtlData<F>; NUM_TABLES],
    challenger: &mut Challenger<F, C::Hasher>,
    timing: &mut TimingTree,
) -> Result<[StarkProof<F, C, D>; NUM_TABLES]>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); RangeCheckStark::<F, D>::COLUMNS]:,
    [(); RangeCheckStark::<F, D>::PUBLIC_INPUTS]:,
    [(); XorStark::<F, D>::COLUMNS]:,
    [(); BitshiftStark::<F, D>::COLUMNS]:,
    [(); ProgramStark::<F, D>::COLUMNS]:,
    [(); MemoryStark::<F, D>::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:, {
    let cpu_proof = prove_single_table::<F, C, CpuStark<F, D>, D>(
        &mozak_stark.cpu_stark,
        config,
        &traces_poly_values[TableKind::Cpu as usize],
        &trace_commitments[TableKind::Cpu as usize],
        [public_inputs.entry_point],
        &ctl_data_per_table[TableKind::Cpu as usize],
        challenger,
        timing,
    )?;

    let rangecheck_proof = prove_single_table::<F, C, RangeCheckStark<F, D>, D>(
        &mozak_stark.rangecheck_stark,
        config,
        &traces_poly_values[TableKind::RangeCheck as usize],
        &trace_commitments[TableKind::RangeCheck as usize],
        [],
        &ctl_data_per_table[TableKind::RangeCheck as usize],
        challenger,
        timing,
    )?;

    let xor_proof = prove_single_table::<F, C, XorStark<F, D>, D>(
        &mozak_stark.xor_stark,
        config,
        &traces_poly_values[TableKind::Xor as usize],
        &trace_commitments[TableKind::Xor as usize],
        [],
        &ctl_data_per_table[TableKind::Xor as usize],
        challenger,
        timing,
    )?;

    let shift_amount_proof = prove_single_table::<F, C, BitshiftStark<F, D>, D>(
        &mozak_stark.shift_amount_stark,
        config,
        &traces_poly_values[TableKind::Bitshift as usize],
        &trace_commitments[TableKind::Bitshift as usize],
        [],
        &ctl_data_per_table[TableKind::Bitshift as usize],
        challenger,
        timing,
    )?;

    let program_proof = prove_single_table::<F, C, ProgramStark<F, D>, D>(
        &mozak_stark.program_stark,
        config,
        &traces_poly_values[TableKind::Program as usize],
        &trace_commitments[TableKind::Program as usize],
        [],
        &ctl_data_per_table[TableKind::Program as usize],
        challenger,
        timing,
    )?;

    let memory_proof = prove_single_table::<F, C, MemoryStark<F, D>, D>(
        &mozak_stark.memory_stark,
        config,
        &traces_poly_values[TableKind::Memory as usize],
        &trace_commitments[TableKind::Memory as usize],
        [],
        &ctl_data_per_table[TableKind::Memory as usize],
        challenger,
        timing,
    )?;

    Ok([
        cpu_proof,
        rangecheck_proof,
        xor_proof,
        shift_amount_proof,
        program_proof,
        memory_proof,
    ])
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;

    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    #[test]
    fn prove_halt() {
        let (program, record) = simple_test_code(&[], &[], &[]);
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_lui() {
        let lui = Instruction {
            op: Op::ADD,
            args: Args {
                rd: 1,
                imm: 0x8000_0000,
                ..Args::default()
            },
        };
        let (program, record) = simple_test_code(&[lui], &[], &[]);
        assert_eq!(record.last_state.get_register_value(1), 0x8000_0000);
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_lui_2() {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 1,
                    imm: 0xDEAD_BEEF,
                    ..Args::default()
                },
            }],
            &[],
            &[],
        );
        assert_eq!(record.last_state.get_register_value(1), 0xDEAD_BEEF,);
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_beq() {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::BEQ,
                args: Args {
                    rs1: 0,
                    rs2: 1,
                    imm: 42, // branch target
                    ..Args::default()
                },
            }],
            &[],
            &[(1, 2)],
        );
        assert_eq!(record.last_state.get_pc(), 8);
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }
}
