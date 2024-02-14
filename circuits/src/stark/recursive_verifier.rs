use std::borrow::Borrow;
use std::fmt::Debug;

use anyhow::Result;
use log::info;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::fri::witness_util::set_fri_proof_target;
use plonky2::gates::exponentiation::ExponentiationGate;
use plonky2::gates::gate::GateRef;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
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

use super::mozak_stark::{all_kind, all_starks, TableKindArray};
use crate::cross_table_lookup::{CrossTableLookup, CtlCheckVarsTarget};
use crate::recproof::unbounded::common_data_for_recursion;
use crate::stark::mozak_stark::{MozakStark, TableKind};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};
use crate::stark::poly::eval_vanishing_poly_circuit;
use crate::stark::proof::{
    AllProof, StarkOpeningSetTarget, StarkProof, StarkProofChallengesTarget, StarkProofTarget,
    StarkProofWithMetadata, StarkProofWithPublicInputsTarget,
};

/// Represents a circuit which recursively verifies STARK proofs.
#[derive(Eq, PartialEq, Debug)]
pub struct MozakStarkVerifierCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    pub circuit: CircuitData<F, C, D>,
    pub targets: TableKindArray<StarkVerifierTargets<F, C, D>>,
}

#[derive(Eq, PartialEq, Debug)]
pub struct StarkVerifierTargets<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    pub stark_proof_with_pis_target: StarkProofWithPublicInputsTarget<D>,
    pub ctl_challenges_target: GrandProductChallengeSet<Target>,
    pub init_challenger_state_target: <C::Hasher as AlgebraicHasher<F>>::AlgebraicPermutation,
    pub zero_target: Target,
}

impl<F, C, const D: usize> StarkVerifierTargets<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>,
{
    pub fn set_targets(
        &self,
        witness: &mut PartialWitness<F>,
        proof_with_metadata: &StarkProofWithMetadata<F, C, D>,
        ctl_challenges: &GrandProductChallengeSet<F>,
    ) {
        set_stark_proof_with_pis_target(
            witness,
            &self.stark_proof_with_pis_target.proof,
            &proof_with_metadata.proof,
            self.zero_target,
        );

        for (challenge_target, challenge) in self
            .ctl_challenges_target
            .challenges
            .iter()
            .zip(&ctl_challenges.challenges)
        {
            witness.set_target(challenge_target.beta, challenge.beta);
            witness.set_target(challenge_target.gamma, challenge.gamma);
        }

        witness.set_target_arr(
            self.init_challenger_state_target.as_ref(),
            proof_with_metadata.init_challenger_state.as_ref(),
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
            self.targets[kind].set_targets(
                &mut inputs,
                &all_proof.proofs_with_metadata[kind],
                &all_proof.ctl_challenges,
            );
        });

        // Set public inputs
        let cpu_target = &self.targets[TableKind::Cpu].stark_proof_with_pis_target;
        inputs.set_target_arr(
            cpu_target.public_inputs.as_ref(),
            all_proof.public_inputs.borrow(),
        );

        self.circuit.prove(inputs)
    }
}

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

    let targets = all_starks!(mozak_stark, |stark, kind| {
        recursive_stark_circuit::<F, C, _, D>(
            &mut builder,
            kind,
            stark,
            degree_bits[kind],
            &mozak_stark.cross_table_lookups,
            inner_config,
        )
    });

    // Register program ROM and memory init trace cap as public inputs.
    for kind in [TableKind::Program, TableKind::ElfMemoryInit] {
        builder.register_public_inputs(
            &targets[kind]
                .stark_proof_with_pis_target
                .proof
                .trace_cap
                .0
                .iter()
                .flat_map(|h| h.elements)
                .collect::<Vec<_>>(),
        );
    }

    add_common_recursion_gates(&mut builder);

    let circuit = builder.build();
    MozakStarkVerifierCircuit { circuit, targets }
}

#[allow(clippy::similar_names)]
/// Returns the recursive Stark circuit.
pub fn recursive_stark_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    table: TableKind,
    stark: &S,
    degree_bits: usize,
    cross_table_lookups: &[CrossTableLookup<F>],
    inner_config: &StarkConfig,
) -> StarkVerifierTargets<F, C, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    let zero_target = builder.zero();

    let num_ctl_zs =
        CrossTableLookup::num_ctl_zs(cross_table_lookups, table, inner_config.num_challenges);
    let stark_proof_with_pis_target =
        add_virtual_stark_proof_with_pis(builder, stark, inner_config, degree_bits, num_ctl_zs);

    let ctl_challenges_target = GrandProductChallengeSet {
        challenges: (0..inner_config.num_challenges)
            .map(|_| GrandProductChallenge {
                beta: builder.add_virtual_target(),
                gamma: builder.add_virtual_target(),
            })
            .collect(),
    };

    let ctl_vars = CtlCheckVarsTarget::from_proof(
        table,
        &stark_proof_with_pis_target.proof,
        cross_table_lookups,
        &ctl_challenges_target,
    );

    let init_challenger_state_target =
        <C::Hasher as AlgebraicHasher<F>>::AlgebraicPermutation::new(std::iter::from_fn(|| {
            Some(builder.add_virtual_target())
        }));
    let mut challenger =
        RecursiveChallenger::<F, C::Hasher, D>::from_state(init_challenger_state_target);
    let challenges = stark_proof_with_pis_target.proof.get_challenges::<F, C>(
        builder,
        &mut challenger,
        inner_config,
    );

    verify_stark_proof_with_challenges_circuit::<F, C, _, D>(
        builder,
        stark,
        &stark_proof_with_pis_target,
        &challenges,
        &ctl_vars,
        inner_config,
    );

    StarkVerifierTargets {
        stark_proof_with_pis_target,
        ctl_challenges_target,
        init_challenger_state_target,
        zero_target,
    }
}

/// Add gates that are sometimes used by recursive circuits, even if it's not
/// actually used by this particular recursive circuit. This is done for
/// uniformity. We sometimes want all recursion circuits to have the same gate
/// set, so that we can do 1-of-n conditional recursion efficiently.
pub fn add_common_recursion_gates<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
) {
    builder.add_gate_to_gate_set(GateRef::new(ExponentiationGate::new_from_config(
        &builder.config,
    )));
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
    ctl_vars: &[CtlCheckVarsTarget<F, D>],
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
        &proof_with_public_inputs.proof.opening_proof,
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
) -> StarkProofWithPublicInputsTarget<D> {
    let proof = add_virtual_stark_proof::<F, S, D>(builder, stark, config, degree_bits, num_ctl_zs);
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
) -> StarkProofTarget<D> {
    let fri_params = config.fri_params(degree_bits);
    let cap_height = fri_params.config.cap_height;

    let num_leaves_per_oracle = vec![
        S::COLUMNS,
        num_ctl_zs,
        stark.quotient_degree_factor() * config.num_challenges,
    ];

    let ctl_zs_cap = builder.add_virtual_cap(cap_height);

    StarkProofTarget {
        trace_cap: builder.add_virtual_cap(cap_height),
        ctl_zs_cap,
        quotient_polys_cap: builder.add_virtual_cap(cap_height),
        openings: add_virtual_stark_opening_set::<F, S, D>(builder, stark, num_ctl_zs, config),
        opening_proof: builder.add_virtual_fri_proof(&num_leaves_per_oracle, &fri_params),
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

    set_fri_proof_target(witness, &proof_target.opening_proof, &proof.opening_proof);
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
        add_common_recursion_gates(&mut builder);
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

pub fn shrink_to_target_degree_bits_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    circuit: &CircuitData<F, C, D>,
    config: &CircuitConfig,
    target_degree_bits: usize,
    proof: &ProofWithPublicInputs<F, C, D>,
) -> Result<(PlonkWrapperCircuit<F, C, D>, ProofWithPublicInputs<F, C, D>)>
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut last_degree_bits = circuit.common.degree_bits();
    assert!(last_degree_bits >= target_degree_bits);

    let mut shrink_circuit = PlonkWrapperCircuit::new(circuit, config.clone());
    let mut shrunk_proof = shrink_circuit.prove(proof)?;
    let shrunk_degree_bits = shrink_circuit.circuit.common.degree_bits();
    info!("shrinking circuit from degree bits {last_degree_bits} to {shrunk_degree_bits}",);
    last_degree_bits = shrunk_degree_bits;

    while last_degree_bits > target_degree_bits {
        shrink_circuit = PlonkWrapperCircuit::new(&shrink_circuit.circuit, config.clone());
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

/// Targets for a VM final proof verification circuit.
pub struct VMVerificationTargets<const D: usize> {
    pub proof_with_pis_target: ProofWithPublicInputsTarget<D>,
    pub vk_target: VerifierCircuitTarget,
}

impl<const D: usize> VMVerificationTargets<D> {
    pub fn new<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>>(
        builder: &mut CircuitBuilder<F, D>,
        public_inputs_size: usize,
        final_recursion_circuit_config: CircuitConfig,
        final_recursion_circuit_degree_bits: usize,
    ) -> VMVerificationTargets<D>
    where
        C::Hasher: AlgebraicHasher<F>, {
        let common_data = common_data_for_recursion::<F, C, D>(
            final_recursion_circuit_config,
            final_recursion_circuit_degree_bits,
            public_inputs_size,
        );

        let proof_with_pis_target = builder.add_virtual_proof_with_pis(&common_data);
        let vk_target = builder.add_virtual_verifier_data(common_data.config.fri_config.cap_height);
        builder.verify_proof::<C>(&proof_with_pis_target, &vk_target, &common_data);

        VMVerificationTargets {
            proof_with_pis_target,
            vk_target,
        }
    }
}

#[must_use]
/// The usual recursion threshold is 2^12 gates, but a few more gates for a
/// constant inner VK and public inputs are used. This pushes the threshold to
/// 2^13. A narrower witness is used as long as the number of gates is below
/// this threshold.
pub fn shrinking_config() -> CircuitConfig {
    CircuitConfig {
        num_routed_wires: 40,
        ..CircuitConfig::standard_recursion_config()
    }
}

pub const FINAL_RECURSION_THRESHOLD: usize = 13;

#[cfg(test)]
mod tests {
    use std::panic;
    use std::panic::AssertUnwindSafe;

    use anyhow::Result;
    use log::info;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::execute_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::iop::witness::{PartialWitness, WitnessWrite};
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;

    use crate::stark::mozak_stark::{MozakStark, PublicInputs};
    use crate::stark::prover::prove;
    use crate::stark::recursive_verifier::{
        recursive_mozak_stark_circuit, shrink_to_target_degree_bits_circuit, shrinking_config,
        VMVerificationTargets,
    };
    use crate::stark::verifier::verify_proof;
    use crate::test_utils::{C, D, F};
    use crate::utils::from_u32;

    type S = MozakStark<F, D>;

    #[test]
    #[ignore]
    fn recursive_verify_mozak_starks() -> Result<()> {
        let stark = S::default();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 1;
        let (program, record) = execute_code(
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
        mozak_stark_circuit.circuit.verify(recursive_proof)
    }

    #[test]
    // #[ignore]
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

        let (program0, record0) = execute_code([inst], &[], &[(6, 100), (7, 200)]);
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

        let (program1, record1) = execute_code(vec![inst; 128], &[], &[(6, 100), (7, 200)]);
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
        // assert_ne!(recursion_degree_bits0, recursion_degree_bits1);
        info!("recursion circuit0 degree bits: {}", recursion_degree_bits0);
        info!("recursion circuit1 degree bits: {}", recursion_degree_bits1);

        let target_degree_bits = 13;
        let (final_circuit0, final_proof0) = shrink_to_target_degree_bits_circuit(
            &recursion_circuit0.circuit,
            &shrinking_config(),
            target_degree_bits,
            &recursion_proof0,
        )?;
        let (final_circuit1, final_proof1) = shrink_to_target_degree_bits_circuit(
            &recursion_circuit1.circuit,
            &shrinking_config(),
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
        let targets = VMVerificationTargets::new::<GoldilocksField, C>(
            &mut builder,
            public_inputs_size,
            shrinking_config(),
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
