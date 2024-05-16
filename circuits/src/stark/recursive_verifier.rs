#![allow(clippy::iter_without_into_iter)]
use std::borrow::Borrow;
use std::fmt::Debug;
use std::marker::PhantomData;

use anyhow::Result;
use itertools::{zip_eq, Itertools};
use log::info;
use mozak_sdk::core::constants::DIGEST_BYTES;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::fri::witness_util::set_fri_proof_target;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::challenger::RecursiveChallenger;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierCircuitTarget};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};
use plonky2::util::reducing::ReducingFactorTarget;
use plonky2::with_context;
use starky::config::StarkConfig;
use starky::constraint_consumer::RecursiveConstraintConsumer;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::{LookupConfig, Stark};

use super::mozak_stark::{all_kind, all_starks, TableKindArray, PUBLIC_TABLE_KINDS};
use crate::columns_view::{columns_view_impl, NumberOfColumns};
use crate::cross_table_lookup::{
    verify_cross_table_lookups_and_public_sub_table_circuit, CrossTableLookup, CtlCheckVarsTarget,
};
use crate::public_sub_table::{
    public_sub_table_values_and_reduced_targets, PublicSubTable, PublicSubTableValuesTarget,
};
use crate::stark::batch_prover::{batch_reduction_arity_bits, sort_degree_bits};
use crate::stark::mozak_stark::{MozakStark, TableKind};
use crate::stark::permutation::challenge::get_grand_product_challenge_set_target;
use crate::stark::poly::eval_vanishing_poly_circuit;
use crate::stark::proof::{
    AllProof, StarkOpeningSetTarget, StarkProof, StarkProofChallengesTarget, StarkProofTarget,
    StarkProofWithPublicInputsTarget,
};

/// Plonky2's recursion threshold is 2^12 gates.
pub const VM_RECURSION_THRESHOLD_DEGREE_BITS: usize = 12;
/// Public inputs (number of Goldilocks elements) using
/// `standard_recursion_config`:
///   `entry_point`: 1
///   `Program trace cap`: 16 (hash count with `cap_height` = 4) * 4 (size of a
///                          hash) = 64
///   `ElfMemoryInit trace cap`: 64
///   `event commitment_tape`: 32
///   `castlist_commitment_tape`: 32
pub const VM_PUBLIC_INPUT_SIZE: usize = VMRecursiveProofPublicInputs::<()>::NUMBER_OF_COLUMNS;
pub const VM_RECURSION_CONFIG: CircuitConfig = CircuitConfig::standard_recursion_config();
pub const VM_RECURSION_CONFIG_NUM_CAPS: usize = 1 << 4;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct VMRecursiveProofPublicInputs<T> {
    pub entry_point: T,
    pub program_trace_cap: [[T; NUM_HASH_OUT_ELTS]; VM_RECURSION_CONFIG_NUM_CAPS],
    pub elf_memory_init_trace_cap: [[T; NUM_HASH_OUT_ELTS]; VM_RECURSION_CONFIG_NUM_CAPS],
    pub event_commitment_tape: [T; DIGEST_BYTES],
    pub castlist_commitment_tape: [T; DIGEST_BYTES],
}

columns_view_impl!(VMRecursiveProofPublicInputs);

#[derive(Eq, PartialEq, Debug)]
pub struct MozakProofTarget<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    pub table_targets: TableKindArray<Option<StarkVerifierTargets<F, C, D>>>,
    pub program_rom_trace_cap_target: MerkleCapTarget,
    pub elf_memory_init_trace_cap_target: MerkleCapTarget,
    pub public_sub_table_values_targets: TableKindArray<Vec<PublicSubTableValuesTarget>>,
    pub batch_targets: Option<StarkVerifierTargets<F, C, D>>,
}

/// Represents a circuit which recursively verifies STARK proofs.
#[derive(Eq, PartialEq, Debug)]
pub struct MozakStarkVerifierCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    pub circuit: CircuitData<F, C, D>,
    pub proof: MozakProofTarget<F, C, D>,
}

#[derive(Eq, PartialEq, Debug)]
pub struct StarkVerifierTargets<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    pub stark_proof_with_pis_target: StarkProofWithPublicInputsTarget<D>,
    pub zero_target: Target,
    pub _f: PhantomData<(F, C)>,
}

impl<F, C, const D: usize> StarkVerifierTargets<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>,
{
    pub fn set_targets(&self, witness: &mut PartialWitness<F>, proof: &StarkProof<F, C, D>) {
        set_stark_proof_with_pis_target(
            witness,
            &self.stark_proof_with_pis_target.proof,
            proof,
            self.zero_target,
        );
    }
}

impl<F, C, const D: usize> MozakStarkVerifierCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>,
{
    pub fn prove(&self, all_proof: &AllProof<F, C, D>) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();

        all_kind!(|kind| {
            self.proof.table_targets[kind]
                .as_ref()
                .expect("")
                .set_targets(&mut inputs, &all_proof.proofs[kind]);

            // set public_sub_table_values targets
            for (public_sub_table_values_target, public_sub_table_values) in zip_eq(
                &self.proof.public_sub_table_values_targets[kind],
                &all_proof.public_sub_table_values[kind],
            ) {
                for (row_target, row) in
                    zip_eq(public_sub_table_values_target, public_sub_table_values)
                {
                    for (&values_target, &values) in zip_eq(row_target, row) {
                        inputs.set_target(values_target, values);
                    }
                }
            }
        });

        // Set public inputs
        let cpu_target = &self.proof.table_targets[TableKind::Cpu]
            .as_ref()
            .expect("")
            .stark_proof_with_pis_target;
        inputs.set_target_arr(
            cpu_target.public_inputs.as_ref(),
            all_proof.public_inputs.borrow(),
        );

        inputs.set_cap_target(
            &self.proof.program_rom_trace_cap_target,
            &all_proof.program_rom_trace_cap,
        );
        inputs.set_cap_target(
            &self.proof.elf_memory_init_trace_cap_target,
            &all_proof.elf_memory_init_trace_cap,
        );

        self.circuit.prove(inputs)
    }
}

fn get_num_columns<F: RichField + Extendable<D>, S: Stark<F, D>, const D: usize>(
    _stark: &S,
) -> usize {
    S::COLUMNS
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn recursive_batch_stark_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    mozak_stark: &MozakStark<F, D>,
    degree_bits: &TableKindArray<usize>,
    public_table_kinds: &[TableKind],
    circuit_config: &CircuitConfig,
    inner_config: &StarkConfig,
)
// -> MozakStarkVerifierCircuit<F, C, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

    let rate_bits = inner_config.fri_config.rate_bits;
    let cap_height = inner_config.fri_config.cap_height;
    let sorted_degree_bits = sort_degree_bits(public_table_kinds, &degree_bits);
    let fri_params = {
        let mut p = inner_config.fri_params(sorted_degree_bits[0]);
        p.reduction_arity_bits =
            batch_reduction_arity_bits(&sorted_degree_bits.clone(), rate_bits, cap_height);
        p
    };

    let mut challenger = RecursiveChallenger::<F, C::Hasher, D>::new(&mut builder);

    let mut num_leaves_per_oracle = [0, 0, 0];
    let stark_proof_with_pis_target = all_starks!(mozak_stark, |stark, kind| {
        let num_ctl_zs = CrossTableLookup::num_ctl_zs(
            &mozak_stark.cross_table_lookups,
            kind,
            inner_config.num_challenges,
        );
        let num_make_row_public_zs = PublicSubTable::num_zs(
            &mozak_stark.public_sub_tables,
            kind,
            inner_config.num_challenges,
        );
        num_leaves_per_oracle[0] += get_num_columns(stark);
        num_leaves_per_oracle[1] += num_ctl_zs + num_make_row_public_zs;
        num_leaves_per_oracle[2] += stark.quotient_degree_factor() * inner_config.num_challenges;
        add_virtual_stark_proof_with_pis(
            &mut builder,
            stark,
            inner_config,
            degree_bits[kind],
            num_ctl_zs + num_make_row_public_zs,
            public_table_kinds.contains(&kind),
        )
    });

    let program_rom_trace_cap_target = builder.add_virtual_cap(inner_config.fri_config.cap_height);
    let elf_memory_init_trace_cap_target =
        builder.add_virtual_cap(inner_config.fri_config.cap_height);
    builder.connect_merkle_caps(
        &stark_proof_with_pis_target[TableKind::Program]
            .proof
            .trace_cap,
        &program_rom_trace_cap_target,
    );
    builder.connect_merkle_caps(
        &stark_proof_with_pis_target[TableKind::ElfMemoryInit]
            .proof
            .trace_cap,
        &elf_memory_init_trace_cap_target,
    );

    let batch_stark_proof_target = StarkProofTarget {
        trace_cap: builder.add_virtual_cap(cap_height),
        ctl_zs_cap: builder.add_virtual_cap(cap_height),
        quotient_polys_cap: builder.add_virtual_cap(cap_height),
        openings: StarkOpeningSetTarget {
            local_values: vec![],
            next_values: vec![],
            ctl_zs: vec![],
            ctl_zs_next: vec![],
            ctl_zs_last: vec![],
            quotient_polys: vec![],
        },
        opening_proof: Some(builder.add_virtual_fri_proof(&num_leaves_per_oracle, &fri_params)),
    };

    for kind in public_table_kinds {
        challenger.observe_cap(&stark_proof_with_pis_target[*kind].proof.trace_cap);
    }
    challenger.observe_cap(&batch_stark_proof_target.trace_cap);

    let ctl_challenges = get_grand_product_challenge_set_target(
        &mut builder,
        &mut challenger,
        inner_config.num_challenges,
    );

    let (public_sub_table_values_targets, reduced_public_sub_table_targets) =
        public_sub_table_values_and_reduced_targets(
            &mut builder,
            &mozak_stark.public_sub_tables,
            &ctl_challenges,
        );

    verify_cross_table_lookups_and_public_sub_table_circuit(
        &mut builder,
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &reduced_public_sub_table_targets,
        &stark_proof_with_pis_target
            .clone()
            .map(|p| p.proof.openings.ctl_zs_last),
        inner_config,
    );

    let table_targets = all_starks!(mozak_stark, |stark, kind| {
        let ctl_vars = CtlCheckVarsTarget::from_proof(
            kind,
            &stark_proof_with_pis_target[kind].proof,
            &mozak_stark.cross_table_lookups,
            &mozak_stark.public_sub_tables,
            &ctl_challenges,
        );

        challenger.compact(&mut builder);
        let challenges_target = stark_proof_with_pis_target[kind]
            .proof
            .get_challenges::<F, C>(&mut builder, &mut challenger, inner_config);

        if public_table_kinds.contains(&kind) {
            verify_stark_proof_with_challenges_circuit::<F, C, _, D>(
                &mut builder,
                stark,
                &stark_proof_with_pis_target[kind],
                &challenges_target,
                &ctl_vars,
                inner_config,
            );
        } else {
            todo!()
        }

        StarkVerifierTargets {
            stark_proof_with_pis_target: stark_proof_with_pis_target[kind].clone(),
            zero_target: builder.zero(),
            _f: PhantomData::<(F, C)>,
        }
    });

    // Register the public tables as public inputs.
    for kind in PUBLIC_TABLE_KINDS {
        builder.register_public_inputs(
            &table_targets[kind]
                .stark_proof_with_pis_target
                .proof
                .trace_cap
                .0
                .iter()
                .flat_map(|h| h.elements)
                .collect::<Vec<_>>(),
        );
    }
    all_kind!(|kind| {
        builder.register_public_inputs(
            &public_sub_table_values_targets[kind]
                .clone()
                .into_iter()
                .flatten()
                .flatten()
                .collect_vec(),
        );
    });
    // let circuit = builder.build();
    // MozakStarkVerifierCircuit {
    //     circuit,
    //     targets,
    //     public_sub_table_values_targets,
    // }
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn recursive_mozak_stark_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    mozak_stark: &MozakStark<F, D>,
    degree_bits: &TableKindArray<usize>,
    circuit_config: &CircuitConfig,
    inner_config: &StarkConfig,
) -> MozakStarkVerifierCircuit<F, C, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

    let mut challenger = RecursiveChallenger::<F, C::Hasher, D>::new(&mut builder);

    let stark_proof_with_pis_target = all_starks!(mozak_stark, |stark, kind| {
        let num_ctl_zs = CrossTableLookup::num_ctl_zs(
            &mozak_stark.cross_table_lookups,
            kind,
            inner_config.num_challenges,
        );
        let num_make_row_public_zs = PublicSubTable::num_zs(
            &mozak_stark.public_sub_tables,
            kind,
            inner_config.num_challenges,
        );
        add_virtual_stark_proof_with_pis(
            &mut builder,
            stark,
            inner_config,
            degree_bits[kind],
            num_ctl_zs + num_make_row_public_zs,
            true,
        )
    });

    let program_rom_trace_cap_target = builder.add_virtual_cap(inner_config.fri_config.cap_height);
    let elf_memory_init_trace_cap_target =
        builder.add_virtual_cap(inner_config.fri_config.cap_height);
    builder.connect_merkle_caps(
        &stark_proof_with_pis_target[TableKind::Program]
            .proof
            .trace_cap,
        &program_rom_trace_cap_target,
    );
    builder.connect_merkle_caps(
        &stark_proof_with_pis_target[TableKind::ElfMemoryInit]
            .proof
            .trace_cap,
        &elf_memory_init_trace_cap_target,
    );

    for pi in &stark_proof_with_pis_target {
        challenger.observe_cap(&pi.proof.trace_cap);
    }

    let ctl_challenges = get_grand_product_challenge_set_target(
        &mut builder,
        &mut challenger,
        inner_config.num_challenges,
    );

    let (public_sub_table_values_targets, reduced_public_sub_table_targets) =
        public_sub_table_values_and_reduced_targets(
            &mut builder,
            &mozak_stark.public_sub_tables,
            &ctl_challenges,
        );

    verify_cross_table_lookups_and_public_sub_table_circuit(
        &mut builder,
        &mozak_stark.cross_table_lookups,
        &mozak_stark.public_sub_tables,
        &reduced_public_sub_table_targets,
        &stark_proof_with_pis_target
            .clone()
            .map(|p| p.proof.openings.ctl_zs_last),
        inner_config,
    );

    let state = challenger.compact(&mut builder);
    let table_targets = all_starks!(mozak_stark, |stark, kind| {
        let ctl_vars = CtlCheckVarsTarget::from_proof(
            kind,
            &stark_proof_with_pis_target[kind].proof,
            &mozak_stark.cross_table_lookups,
            &mozak_stark.public_sub_tables,
            &ctl_challenges,
        );

        let mut challenger = RecursiveChallenger::from_state(state);
        let challenges_target = stark_proof_with_pis_target[kind]
            .proof
            .get_challenges::<F, C>(&mut builder, &mut challenger, inner_config);

        verify_stark_proof_with_challenges_circuit::<F, C, _, D>(
            &mut builder,
            stark,
            &stark_proof_with_pis_target[kind],
            &challenges_target,
            &ctl_vars,
            inner_config,
        );

        Some(StarkVerifierTargets {
            stark_proof_with_pis_target: stark_proof_with_pis_target[kind].clone(),
            zero_target: builder.zero(),
            _f: PhantomData,
        })
    });

    // Register the public tables as public inputs.
    for kind in PUBLIC_TABLE_KINDS {
        builder.register_public_inputs(
            &table_targets[kind]
                .as_ref()
                .expect("")
                .stark_proof_with_pis_target
                .proof
                .trace_cap
                .0
                .iter()
                .flat_map(|h| h.elements)
                .collect::<Vec<_>>(),
        );
    }
    all_kind!(|kind| {
        builder.register_public_inputs(
            &public_sub_table_values_targets[kind]
                .clone()
                .into_iter()
                .flatten()
                .flatten()
                .collect_vec(),
        );
    });

    let circuit = builder.build();
    MozakStarkVerifierCircuit {
        circuit,
        proof: MozakProofTarget {
            table_targets,
            program_rom_trace_cap_target,
            elf_memory_init_trace_cap_target,
            public_sub_table_values_targets,
            batch_targets: None,
        },
    }
}

/// Recursively verifies an inner proof.
fn verify_stark_proof_with_challenges_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    proof_with_public_inputs: &StarkProofWithPublicInputsTarget<D>,
    challenges: &StarkProofChallengesTarget<D>,
    ctl_vars: &[CtlCheckVarsTarget<D>],
    inner_config: &StarkConfig,
) where
    C::Hasher: AlgebraicHasher<F>, {
    let zero = builder.zero();
    let one = builder.one_extension();

    let StarkOpeningSetTarget {
        local_values,
        next_values,
        ctl_zs: _,
        ctl_zs_next: _,
        ctl_zs_last,
        quotient_polys,
    } = &proof_with_public_inputs.proof.openings;

    let converted_public_inputs: Vec<ExtensionTarget<D>> = proof_with_public_inputs
        .public_inputs
        .iter()
        .map(|target| builder.convert_to_ext(*target)) // replace with actual conversion function/method
        .collect();

    let vars =
        S::EvaluationFrameTarget::from_values(local_values, next_values, &converted_public_inputs);

    let degree_bits = proof_with_public_inputs
        .proof
        .recover_degree_bits(inner_config);
    let zeta_pow_deg = builder.exp_power_of_2_extension(challenges.stark_zeta, degree_bits);
    let z_h_zeta = builder.sub_extension(zeta_pow_deg, one);
    let (l_0, l_last) =
        eval_l_0_and_l_last_circuit(builder, degree_bits, challenges.stark_zeta, z_h_zeta);
    let last =
        builder.constant_extension(F::Extension::primitive_root_of_unity(degree_bits).inverse());
    let z_last = builder.sub_extension(challenges.stark_zeta, last);

    let mut consumer = RecursiveConstraintConsumer::<F, D>::new(
        builder.zero_extension(),
        challenges.stark_alphas.clone(),
        z_last,
        l_0,
        l_last,
    );

    with_context!(
        builder,
        "evaluate vanishing polynomial",
        eval_vanishing_poly_circuit::<F, S, D>(builder, stark, &vars, ctl_vars, &mut consumer,)
    );
    let vanishing_polys_zeta = consumer.accumulators();

    // Check each polynomial identity, of the form `vanishing(x) = Z_H(x)
    // quotient(x)`, at zeta.
    let mut scale = ReducingFactorTarget::new(zeta_pow_deg);
    for (i, chunk) in quotient_polys
        .chunks(stark.quotient_degree_factor())
        .enumerate()
    {
        let recombined_quotient = scale.reduce(chunk, builder);
        let computed_vanishing_poly = builder.mul_extension(z_h_zeta, recombined_quotient);
        builder.connect_extension(vanishing_polys_zeta[i], computed_vanishing_poly);
    }

    let merkle_caps = vec![
        proof_with_public_inputs.proof.trace_cap.clone(),
        proof_with_public_inputs.proof.ctl_zs_cap.clone(),
        proof_with_public_inputs.proof.quotient_polys_cap.clone(),
    ];

    let fri_instance = stark.fri_instance_target(
        builder,
        challenges.stark_zeta,
        F::primitive_root_of_unity(degree_bits),
        0,
        0,
        inner_config,
        Some(&LookupConfig {
            degree_bits,
            num_zs: ctl_zs_last.len(),
        }),
    );
    builder.verify_fri_proof::<C>(
        &fri_instance,
        &proof_with_public_inputs
            .proof
            .openings
            .to_fri_openings(zero),
        &challenges.fri_challenges,
        &merkle_caps,
        &proof_with_public_inputs
            .proof
            .opening_proof
            .as_ref()
            .expect("Expected opening_proof to be Some"),
        &inner_config.fri_params(degree_bits),
    );
}

fn eval_l_0_and_l_last_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    log_n: usize,
    x: ExtensionTarget<D>,
    z_x: ExtensionTarget<D>,
) -> (ExtensionTarget<D>, ExtensionTarget<D>) {
    let n = builder.constant_extension(F::Extension::from_canonical_usize(1 << log_n));
    let g = builder.constant_extension(F::Extension::primitive_root_of_unity(log_n));
    let one = builder.one_extension();
    let l_0_deno = builder.mul_sub_extension(n, x, n);
    let l_last_deno = builder.mul_sub_extension(g, x, one);
    let l_last_deno = builder.mul_extension(n, l_last_deno);

    (
        builder.div_extension(z_x, l_0_deno),
        builder.div_extension(z_x, l_last_deno),
    )
}

pub fn add_virtual_stark_proof_with_pis<
    F: RichField + Extendable<D>,
    S: Stark<F, D>,
    const D: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    config: &StarkConfig,
    degree_bits: usize,
    num_ctl_zs: usize,
    add_opening_proof: bool,
) -> StarkProofWithPublicInputsTarget<D> {
    let proof = add_virtual_stark_proof::<F, S, D>(
        builder,
        stark,
        config,
        degree_bits,
        num_ctl_zs,
        add_opening_proof,
    );
    let public_inputs = builder.add_virtual_targets(S::PUBLIC_INPUTS);
    builder.register_public_inputs(&public_inputs);
    StarkProofWithPublicInputsTarget {
        proof,
        public_inputs,
    }
}

pub fn add_virtual_stark_proof<F: RichField + Extendable<D>, S: Stark<F, D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    config: &StarkConfig,
    degree_bits: usize,
    num_ctl_zs: usize,
    add_opening_proof: bool,
) -> StarkProofTarget<D> {
    let fri_params = config.fri_params(degree_bits);
    let cap_height = fri_params.config.cap_height;

    let num_leaves_per_oracle = vec![
        S::COLUMNS,
        num_ctl_zs,
        stark.quotient_degree_factor() * config.num_challenges,
    ];

    let ctl_zs_cap = builder.add_virtual_cap(cap_height);
    // TODO: we can remove it without affecting the number of degrees
    let opening_proof = add_opening_proof
        .then(|| builder.add_virtual_fri_proof(&num_leaves_per_oracle, &fri_params));

    StarkProofTarget {
        trace_cap: builder.add_virtual_cap(cap_height),
        ctl_zs_cap,
        quotient_polys_cap: builder.add_virtual_cap(cap_height),
        openings: add_virtual_stark_opening_set::<F, S, D>(builder, stark, num_ctl_zs, config),
        opening_proof,
    }
}

fn add_virtual_stark_opening_set<F: RichField + Extendable<D>, S: Stark<F, D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    num_ctl_zs: usize,
    config: &StarkConfig,
) -> StarkOpeningSetTarget<D> {
    let num_challenges = config.num_challenges;
    StarkOpeningSetTarget {
        local_values: builder.add_virtual_extension_targets(S::COLUMNS),
        next_values: builder.add_virtual_extension_targets(S::COLUMNS),
        ctl_zs: builder.add_virtual_extension_targets(num_ctl_zs),
        ctl_zs_next: builder.add_virtual_extension_targets(num_ctl_zs),
        ctl_zs_last: builder.add_virtual_targets(num_ctl_zs),
        quotient_polys: builder
            .add_virtual_extension_targets(stark.quotient_degree_factor() * num_challenges),
    }
}

pub fn set_stark_proof_with_pis_target<F, C: GenericConfig<D, F = F>, W, const D: usize>(
    witness: &mut W,
    proof_target: &StarkProofTarget<D>,
    proof: &StarkProof<F, C, D>,
    zero: Target,
) where
    F: RichField + Extendable<D>,
    C::Hasher: AlgebraicHasher<F>,
    W: Witness<F>, {
    witness.set_cap_target(&proof_target.trace_cap, &proof.trace_cap);
    witness.set_cap_target(&proof_target.quotient_polys_cap, &proof.quotient_polys_cap);

    witness.set_fri_openings(
        &proof_target.openings.to_fri_openings(zero),
        &proof.openings.to_fri_openings(),
    );

    witness.set_cap_target(&proof_target.ctl_zs_cap, &proof.ctl_zs_cap);

    proof_target.opening_proof.as_ref().map(|opening_proof| {
        set_fri_proof_target(witness, opening_proof, &proof.opening_proof);
    });
}

// Generates `CircuitData` usable for recursion.
#[must_use]
pub fn circuit_data_for_recursion<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    config: &CircuitConfig,
    target_degree_bits: usize,
    public_input_size: usize,
) -> CircuitData<F, C, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    // Generate a simple circuit that will be recursively verified in the out
    // circuit.
    let common = {
        let mut builder = CircuitBuilder::<F, D>::new(config.clone());
        while builder.num_gates() < 1 << 5 {
            builder.add_gate(NoopGate, vec![]);
        }
        builder.build::<C>().common
    };

    let mut builder = CircuitBuilder::<F, D>::new(config.clone());
    let proof = builder.add_virtual_proof_with_pis(&common);
    let verifier_data = builder.add_virtual_verifier_data(common.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof, &verifier_data, &common);
    for _ in 0..public_input_size {
        builder.add_virtual_public_input();
    }
    // We don't want to pad all the way up to 2^target_degree_bits, as the builder
    // will add a few special gates afterward. So just pad to
    // 2^(target_degree_bits - 1) + 1. Then the builder will pad to the next
    // power of two.
    let min_gates = (1 << (target_degree_bits - 1)) + 1;
    while builder.num_gates() < min_gates {
        builder.add_gate(NoopGate, vec![]);
    }
    builder.build::<C>()
}

/// Represents a circuit which recursively verifies a PLONK proof.
#[derive(Eq, PartialEq, Debug)]
pub struct PlonkWrapperCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub circuit: CircuitData<F, C, D>,
    pub proof_with_pis_target: ProofWithPublicInputsTarget<D>,
}

impl<F, C, const D: usize> PlonkWrapperCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>,
{
    pub fn new(
        circuit: &CircuitData<F, C, D>,
        config: CircuitConfig,
    ) -> PlonkWrapperCircuit<F, C, D> {
        let mut builder = CircuitBuilder::new(config);
        let proof_with_pis_target = builder.add_virtual_proof_with_pis(&circuit.common);
        let last_vk = builder.constant_verifier_data(&circuit.verifier_only);
        builder.verify_proof::<C>(&proof_with_pis_target, &last_vk, &circuit.common);
        builder.register_public_inputs(&proof_with_pis_target.public_inputs); // carry PIs forward
        let circuit = builder.build::<C>();
        PlonkWrapperCircuit {
            circuit,
            proof_with_pis_target,
        }
    }

    pub fn prove(
        &self,
        proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        inputs.set_proof_with_pis_target(&self.proof_with_pis_target, proof);
        self.circuit.prove(inputs)
    }
}

/// Shrinks a PLONK circuit to the target degree bits.
pub fn shrink_to_target_degree_bits_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    circuit: &CircuitData<F, C, D>,
    shrink_config: &CircuitConfig,
    target_degree_bits: usize,
    proof: &ProofWithPublicInputs<F, C, D>,
) -> Result<(PlonkWrapperCircuit<F, C, D>, ProofWithPublicInputs<F, C, D>)>
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut last_degree_bits = circuit.common.degree_bits();
    assert!(last_degree_bits >= target_degree_bits);

    let mut shrink_circuit = PlonkWrapperCircuit::new(circuit, shrink_config.clone());
    let mut shrunk_proof = shrink_circuit.prove(proof)?;
    let shrunk_degree_bits = shrink_circuit.circuit.common.degree_bits();
    info!("shrinking circuit from degree bits {last_degree_bits} to {shrunk_degree_bits}",);
    last_degree_bits = shrunk_degree_bits;

    while last_degree_bits > target_degree_bits {
        shrink_circuit = PlonkWrapperCircuit::new(&shrink_circuit.circuit, shrink_config.clone());
        let shrunk_degree_bits = shrink_circuit.circuit.common.degree_bits();
        assert!(
            shrunk_degree_bits < last_degree_bits,
            "shrink failed at degree bits: {last_degree_bits}",
        );
        info!("shrinking circuit from degree bits {last_degree_bits} to {shrunk_degree_bits}",);
        last_degree_bits = shrunk_degree_bits;
        shrunk_proof = shrink_circuit.prove(&shrunk_proof)?;
    }
    assert_eq!(last_degree_bits, target_degree_bits);

    Ok((shrink_circuit, shrunk_proof))
}

/// Targets for a recursive VM proof verification circuit.
pub struct VMVerificationTargets<const D: usize> {
    pub proof_with_pis_target: ProofWithPublicInputsTarget<D>,
    pub vk_target: VerifierCircuitTarget,
}

/// Verifies a recursive VM proof. Caller should also verify the program hash
/// and vk to ensure that the proof is from the correct program.
pub fn verify_recursive_vm_proof<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    public_inputs_size: usize,
    recursion_config: &CircuitConfig,
    recursion_degree_bits: usize,
) -> VMVerificationTargets<D>
where
    C::Hasher: AlgebraicHasher<F>, {
    let common_data = circuit_data_for_recursion::<F, C, D>(
        recursion_config,
        recursion_degree_bits,
        public_inputs_size,
    )
    .common;

    let proof_with_pis_target = builder.add_virtual_proof_with_pis(&common_data);
    let vk_target = builder.add_virtual_verifier_data(common_data.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof_with_pis_target, &vk_target, &common_data);

    VMVerificationTargets {
        proof_with_pis_target,
        vk_target,
    }
}

#[cfg(test)]
mod tests {

    use std::panic;
    use std::panic::AssertUnwindSafe;

    use anyhow::Result;
    use log::info;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_sdk::core::constants::DIGEST_BYTES;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;
    use plonky2::iop::witness::{PartialWitness, WitnessWrite};
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;

    use crate::stark::batch_prover::batch_prove;
    use crate::stark::batch_verifier::batch_verify_proof;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs, PUBLIC_TABLE_KINDS};
    use crate::stark::prover::prove;
    use crate::stark::recursive_verifier::{
        recursive_batch_stark_circuit, recursive_mozak_stark_circuit,
        shrink_to_target_degree_bits_circuit, verify_recursive_vm_proof,
        VMRecursiveProofPublicInputs, VM_PUBLIC_INPUT_SIZE, VM_RECURSION_CONFIG,
        VM_RECURSION_THRESHOLD_DEGREE_BITS,
    };
    use crate::stark::verifier::verify_proof;
    use crate::test_utils::{C, D, F};
    use crate::utils::from_u32;

    type S = MozakStark<F, D>;

    #[test]
    fn recursive_verify_mozak_starks() -> Result<()> {
        let stark = S::default();
        let config = StarkConfig::standard_fast_config();
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
            &[(6, 100), (7, 200)],
        );
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let mozak_proof = prove::<F, C, D>(
            &program,
            &record,
            &stark,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        )?;
        verify_proof(&stark, mozak_proof.clone(), &config)?;

        let circuit_config = CircuitConfig::standard_recursion_config();
        let mozak_stark_circuit = recursive_mozak_stark_circuit::<F, C, D>(
            &stark,
            &mozak_proof.degree_bits(&config),
            &circuit_config,
            &config,
        );

        let recursive_proof = mozak_stark_circuit.prove(&mozak_proof)?;
        let public_input_slice: [F; VM_PUBLIC_INPUT_SIZE] =
            recursive_proof.public_inputs.as_slice().try_into().unwrap();
        let expected_event_commitment_tape = [F::ZERO; DIGEST_BYTES];
        let expected_castlist_commitment_tape = [F::ZERO; DIGEST_BYTES];
        let recursive_proof_public_inputs: &VMRecursiveProofPublicInputs<F> =
            &public_input_slice.into();
        assert_eq!(
            recursive_proof_public_inputs.event_commitment_tape, expected_event_commitment_tape,
            "Could not find expected_event_commitment_tape in recursive proof's public inputs"
        );
        assert_eq!(
            recursive_proof_public_inputs.castlist_commitment_tape,
            expected_castlist_commitment_tape,
            "Could not find expected_castlist_commitment_tape in recursive proof's public inputs"
        );

        mozak_stark_circuit.circuit.verify(recursive_proof)
    }

    #[test]
    fn recursive_verify_batch_starks() -> Result<()> {
        let stark = S::default();
        let config = StarkConfig::standard_fast_config();
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
            &[(6, 100), (7, 200)],
        );
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let (mozak_proof, degree_bits) = batch_prove::<F, C, D>(
            &program,
            &record,
            &stark,
            &PUBLIC_TABLE_KINDS,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        )?;
        batch_verify_proof(
            &stark,
            &PUBLIC_TABLE_KINDS,
            mozak_proof.clone(),
            &config,
            &degree_bits,
        )?;

        let circuit_config = CircuitConfig::standard_recursion_config();
        // let mozak_stark_circuit = recursive_batch_stark_circuit::<F, C, D>(
        //     &stark,
        //     &degree_bits,
        //     &PUBLIC_TABLE_KINDS,
        //     &circuit_config,
        //     &config,
        // );

        // let recursive_proof = mozak_stark_circuit.prove(&mozak_proof)?;
        // let public_input_slice: [F; VM_PUBLIC_INPUT_SIZE] =
        //     recursive_proof.public_inputs.as_slice().try_into().unwrap();
        // let expected_event_commitment_tape = [F::ZERO; DIGEST_BYTES];
        // let expected_castlist_commitment_tape = [F::ZERO; DIGEST_BYTES];
        // let recursive_proof_public_inputs: &VMRecursiveProofPublicInputs<F> =
        //     &public_input_slice.into();
        // assert_eq!(
        //     recursive_proof_public_inputs.event_commitment_tape,
        // expected_event_commitment_tape,     "Could not find
        // expected_event_commitment_tape in recursive proof's public inputs" );
        // assert_eq!(
        //     recursive_proof_public_inputs.castlist_commitment_tape,
        //     expected_castlist_commitment_tape,
        //     "Could not find expected_castlist_commitment_tape in recursive proof's
        // public inputs" );
        //
        // mozak_stark_circuit.circuit.verify(recursive_proof)

        Ok(())
    }

    #[test]
    #[ignore]
    #[allow(clippy::too_many_lines)]
    fn same_circuit_verify_different_vm_proofs() -> Result<()> {
        let stark = S::default();
        let inst = Instruction {
            op: Op::ADD,
            args: Args {
                rd: 5,
                rs1: 6,
                rs2: 7,
                ..Args::default()
            },
        };

        let (program0, record0) = code::execute([inst], &[], &[(6, 100), (7, 200)]);
        let public_inputs = PublicInputs {
            entry_point: from_u32(program0.entry_point),
        };
        let stark_config0 = StarkConfig::standard_fast_config();
        let mozak_proof0 = prove::<F, C, D>(
            &program0,
            &record0,
            &stark,
            &stark_config0,
            public_inputs,
            &mut TimingTree::default(),
        )?;

        let (program1, record1) = code::execute(vec![inst; 128], &[], &[(6, 100), (7, 200)]);
        let public_inputs = PublicInputs {
            entry_point: from_u32(program1.entry_point),
        };
        let stark_config1 = StarkConfig::standard_fast_config();
        let mozak_proof1 = prove::<F, C, D>(
            &program1,
            &record1,
            &stark,
            &stark_config1,
            public_inputs,
            &mut TimingTree::default(),
        )?;

        // The degree bits should be different for the two proofs.
        assert_ne!(
            mozak_proof0.degree_bits(&stark_config0),
            mozak_proof1.degree_bits(&stark_config1)
        );

        let recursion_circuit_config = CircuitConfig::standard_recursion_config();
        let recursion_circuit0 = recursive_mozak_stark_circuit::<F, C, D>(
            &stark,
            &mozak_proof0.degree_bits(&stark_config0),
            &recursion_circuit_config,
            &stark_config0,
        );
        let recursion_proof0 = recursion_circuit0.prove(&mozak_proof0)?;

        let recursion_circuit1 = recursive_mozak_stark_circuit::<F, C, D>(
            &stark,
            &mozak_proof1.degree_bits(&stark_config1),
            &recursion_circuit_config,
            &stark_config1,
        );
        let recursion_proof1 = recursion_circuit1.prove(&mozak_proof1)?;

        recursion_circuit0
            .circuit
            .verify(recursion_proof0.clone())?;

        let public_inputs_size = recursion_proof0.public_inputs.len();
        assert_eq!(VM_PUBLIC_INPUT_SIZE, public_inputs_size);
        assert_eq!(public_inputs_size, recursion_proof1.public_inputs.len());

        // It is not possible to verify different VM proofs with the same recursion
        // circuit.
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            recursion_circuit0
                .circuit
                .verify(recursion_proof1.clone())
                .expect("Verification failed");
        }));
        assert!(result.is_err(), "Verification did not failed as expected");

        let recursion_degree_bits0 = recursion_circuit0.circuit.common.degree_bits();
        let recursion_degree_bits1 = recursion_circuit1.circuit.common.degree_bits();
        assert_ne!(recursion_degree_bits0, recursion_degree_bits1);
        info!("recursion circuit0 degree bits: {}", recursion_degree_bits0);
        info!("recursion circuit1 degree bits: {}", recursion_degree_bits1);

        let target_degree_bits = VM_RECURSION_THRESHOLD_DEGREE_BITS;
        let (final_circuit0, final_proof0) = shrink_to_target_degree_bits_circuit(
            &recursion_circuit0.circuit,
            &VM_RECURSION_CONFIG,
            target_degree_bits,
            &recursion_proof0,
        )?;
        let (final_circuit1, final_proof1) = shrink_to_target_degree_bits_circuit(
            &recursion_circuit1.circuit,
            &VM_RECURSION_CONFIG,
            target_degree_bits,
            &recursion_proof1,
        )?;
        assert_eq!(
            final_circuit0.circuit.common.degree_bits(),
            target_degree_bits
        );
        assert_eq!(
            final_circuit1.circuit.common.degree_bits(),
            target_degree_bits
        );

        final_circuit0.circuit.verify(final_proof0.clone())?;
        final_circuit1.circuit.verify(final_proof1.clone())?;

        // It is still not possible to verify different VM proofs with the same
        // recursion circuit at this point. But the final proofs now have the same
        // degree bits.
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            final_circuit0
                .circuit
                .verify(final_proof1.clone())
                .expect("Verification failed");
        }));
        assert!(result.is_err(), "Verification did not failed as expected");

        // Let's build a circuit to verify the final proofs.
        let mut builder = CircuitBuilder::new(CircuitConfig::standard_recursion_config());
        let targets = verify_recursive_vm_proof::<GoldilocksField, C, D>(
            &mut builder,
            public_inputs_size,
            &VM_RECURSION_CONFIG,
            target_degree_bits,
        );
        let circuit = builder.build::<C>();

        // This time, we can verify the final proofs from two different VM programs in
        // the same circuit.
        let mut pw = PartialWitness::new();
        pw.set_proof_with_pis_target(&targets.proof_with_pis_target, &final_proof0);
        pw.set_verifier_data_target(&targets.vk_target, &final_circuit0.circuit.verifier_only);
        let proof = circuit.prove(pw)?;
        circuit.verify(proof)?;

        let mut pw = PartialWitness::new();
        pw.set_proof_with_pis_target(&targets.proof_with_pis_target, &final_proof1);
        pw.set_verifier_data_target(&targets.vk_target, &final_circuit1.circuit.verifier_only);
        let proof = circuit.prove(pw)?;
        circuit.verify(proof)?;

        Ok(())
    }
}
