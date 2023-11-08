#![allow(clippy::too_many_lines)]

use std::fmt::Display;

use anyhow::{ensure, Result};
use itertools::Itertools;
use log::log_enabled;
use log::Level::Debug;
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::field::extension::Extendable;
use plonky2::field::packable::Packable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::GenericConfig;
use plonky2::timed;
use plonky2::util::log2_strict;
use plonky2::util::timing::TimingTree;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use starky::config::StarkConfig;
use starky::stark::{LookupConfig, Stark};

use super::mozak_stark::{MozakStark, TableKind, NUM_TABLES};
use super::proof::{AllProof, StarkOpeningSet, StarkProof};
use crate::cross_table_lookup::ctl_utils::debug_ctl;
use crate::cross_table_lookup::{cross_table_lookup_data, CtlData};
use crate::generation::{debug_traces, generate_traces};
use crate::stark::mozak_stark::PublicInputs;
use crate::stark::permutation::challenge::{GrandProductChallengeSet, GrandProductChallengeTrait};
use crate::stark::permutation::compute_permutation_z_polys;
use crate::stark::poly::compute_quotient_polys;
use crate::stark::proof::StarkProofWithMetadata;

pub fn prove<F, C, const D: usize>(
    program: &Program,
    record: &ExecutionRecord<F>,
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    public_inputs: PublicInputs<F>,
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    let traces_poly_values = generate_traces(program, record);
    if mozak_stark.debug || std::env::var("MOZAK_STARK_DEBUG").is_ok() {
        debug_traces(&traces_poly_values, mozak_stark, &public_inputs);
        debug_ctl(&traces_poly_values, mozak_stark);
    }
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
    C: GenericConfig<D, F = F>, {
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
    let proofs_with_metadata = timed!(
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
    let memory_init_trace_cap = trace_caps[TableKind::MemoryInit as usize].clone();
    if log_enabled!(Debug) {
        timing.print();
    }
    Ok(AllProof {
        proofs_with_metadata,
        ctl_challenges,
        program_rom_trace_cap,
        memory_init_trace_cap,
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
    public_inputs: &[F],
    ctl_data: &CtlData<F>,
    challenger: &mut Challenger<F, C::Hasher>,
    timing: &mut TimingTree,
) -> Result<StarkProofWithMetadata<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D> + Display, {
    let degree = trace_poly_values[0].len();
    let degree_bits = log2_strict(degree);
    let fri_params = config.fri_params(degree_bits);
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;
    assert!(
        fri_params.total_arities() <= degree_bits + rate_bits - cap_height,
        "FRI total reduction arity is too large.",
    );

    let init_challenger_state = challenger.compact();

    // Permutation arguments.
    let permutation_challenges: Vec<GrandProductChallengeSet<F>> = challenger
        .get_n_grand_product_challenge_sets(config.num_challenges, stark.permutation_batch_size());
    let mut permutation_zs = timed!(
        timing,
        format!("{stark}: compute permutation Z(x) polys").as_str(),
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
        format!("{stark}: compute Zs commitment").as_str(),
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
        format!("{stark}: compute quotient polynomial").as_str(),
        compute_quotient_polys::<F, <F as Packable>::Packing, C, S, D>(
            stark,
            trace_commitment,
            &permutation_ctl_zs_commitment,
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
    let quotient_commitment = timed!(
        timing,
        format!("{stark}: compute quotient commitment").as_str(),
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
        format!("{stark}: compute opening proofs").as_str(),
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

    let proof = StarkProof {
        trace_cap: trace_commitment.merkle_tree.cap.clone(),
        permutation_ctl_zs_cap,
        quotient_polys_cap,
        openings,
        opening_proof,
    };
    Ok(StarkProofWithMetadata {
        init_challenger_state,
        proof,
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
) -> Result<[StarkProofWithMetadata<F, C, D>; NUM_TABLES]>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    macro_rules! make_proof {
        ($stark: expr, $kind: expr, $public_inputs: expr) => {
            prove_single_table(
                &$stark,
                config,
                &traces_poly_values[$kind as usize],
                &trace_commitments[$kind as usize],
                &$public_inputs,
                &ctl_data_per_table[$kind as usize],
                challenger,
                timing,
            )
        };
    }

    Ok([
        make_proof!(mozak_stark.cpu_stark, TableKind::Cpu, [
            public_inputs.entry_point
        ])?,
        make_proof!(mozak_stark.rangecheck_stark, TableKind::RangeCheck, [])?,
        make_proof!(mozak_stark.xor_stark, TableKind::Xor, [])?,
        make_proof!(mozak_stark.shift_amount_stark, TableKind::Bitshift, [])?,
        make_proof!(mozak_stark.program_stark, TableKind::Program, [])?,
        make_proof!(mozak_stark.memory_stark, TableKind::Memory, [])?,
        make_proof!(mozak_stark.memory_init_stark, TableKind::MemoryInit, [])?,
        make_proof!(
            mozak_stark.rangecheck_limb_stark,
            TableKind::RangeCheckLimb,
            []
        )?,
        make_proof!(
            mozak_stark.halfword_memory_stark,
            TableKind::HalfWordMemory,
            []
        )?,
        make_proof!(
            mozak_stark.fullword_memory_stark,
            TableKind::FullWordMemory,
            []
        )?,
        make_proof!(mozak_stark.register_init_stark, TableKind::RegisterInit, [])?,
        make_proof!(mozak_stark.register_stark, TableKind::Register, [])?,
        make_proof!(mozak_stark.io_memory_stark, TableKind::IoMemory, [])?,
        make_proof!(
            mozak_stark.poseidon2_sponge_stark,
            TableKind::Poseidon2Sponge,
            []
        )?,
        make_proof!(mozak_stark.poseidon2_stark, TableKind::Poseidon2, [])?,
    ])
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use itertools::izip;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use mozak_system::system::ecall;
    use mozak_system::system::reg_abi::{REG_A0, REG_A1, REG_A2, REG_A3};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;
    use plonky2::hash::poseidon2::Poseidon2Hash;
    use plonky2::plonk::config::{GenericHashOut, Hasher};

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

    struct Poseidon2Test {
        pub data: String,
        pub input_start_addr: u32,
        pub output_start_addr: u32,
    }

    fn test_poseidon2(test_data: &[Poseidon2Test]) {
        let mut instructions = vec![];
        let mut memory: Vec<(u32, u8)> = vec![];

        for test_datum in test_data {
            let mut data_bytes = test_datum.data.as_bytes().to_vec();
            // VM expects input len to be multiple of RATE bits
            data_bytes.resize(data_bytes.len().next_multiple_of(8), 0_u8);
            let data_len = data_bytes.len();
            let input_memory: Vec<(u32, u8)> =
                izip!((test_datum.input_start_addr..), data_bytes).collect();
            memory.extend(input_memory);
            instructions.extend(&[
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A0,
                        imm: ecall::POSEIDON2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A1,
                        imm: test_datum.input_start_addr,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A2,
                        imm: u32::try_from(data_len).expect("don't use very long data"),
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A3,
                        imm: test_datum.output_start_addr,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ECALL,
                    ..Default::default()
                },
            ]);
        }

        let (program, record) = simple_test_code(&instructions, memory.as_slice(), &[]);
        for test_datum in test_data {
            let output: Vec<u8> = (0..32_u8)
                .map(|i| {
                    record
                        .last_state
                        .load_u8(test_datum.output_start_addr + u32::from(i))
                })
                .collect();
            let mut data_bytes = test_datum.data.as_bytes().to_vec();
            // VM expects input len to be multiple of RATE bits
            data_bytes.resize(data_bytes.len().next_multiple_of(8), 0_u8);
            let data_fields: Vec<GoldilocksField> = data_bytes
                .iter()
                .map(|x| GoldilocksField::from_canonical_u8(*x))
                .collect();
            assert_eq!(output, Poseidon2Hash::hash_no_pad(&data_fields).to_bytes());
        }
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_poseidon2() {
        test_poseidon2(&[Poseidon2Test {
            data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }]);
        test_poseidon2(&[Poseidon2Test {
            data: "😇 Mozak is knowledge arguments based technology".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }]);
        test_poseidon2(&[
            Poseidon2Test {
                data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
                input_start_addr: 512,
                output_start_addr: 1024,
            },
            Poseidon2Test {
                data: "😇 Mozak is knowledge arguments based technology".to_string(),
                input_start_addr: 1024 + 32, /* make sure input and output do not overlap with
                                              * earlier call */
                output_start_addr: 2048,
            },
        ]);
    }
}
