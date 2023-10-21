use std::fmt::Debug;

use anyhow::{ensure, Result};
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::fri::witness_util::set_fri_proof_target;
use plonky2::gates::exponentiation::ExponentiationGate;
use plonky2::gates::gate::GateRef;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::iop::challenger::{Challenger, RecursiveChallenger};
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;
use plonky2::util::log2_ceil;
use plonky2::util::reducing::ReducingFactorTarget;
use plonky2::with_context;
use starky::config::StarkConfig;
use starky::constraint_consumer::RecursiveConstraintConsumer;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::{LookupConfig, Stark};

use crate::cross_table_lookup::{verify_cross_table_lookups, CrossTableLookup, CtlCheckVarsTarget};
use crate::stark::mozak_stark::{TableKind, NUM_TABLES};
use crate::stark::permutation::challenge::{
    GrandProductChallenge, GrandProductChallengeSet, GrandProductChallengeTrait,
};
use crate::stark::permutation::PermutationCheckDataTarget;
use crate::stark::poly::eval_vanishing_poly_circuit;
use crate::stark::proof::{
    StarkOpeningSetTarget, StarkProof, StarkProofChallengesTarget, StarkProofTarget,
    StarkProofWithMetadata, StarkProofWithPublicInputsTarget,
};

/// Table-wise recursive proofs of an `AllProof`.
pub struct RecursiveAllProof<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
> {
    pub recursive_proofs: [ProofWithPublicInputs<F, C, D>; NUM_TABLES],
}

pub(crate) struct PublicInputs<T: Copy + Default + Eq + PartialEq + Debug, P: PlonkyPermutation<T>>
{
    pub(crate) trace_cap: Vec<Vec<T>>,
    pub(crate) ctl_zs_last: Vec<T>,
    pub(crate) ctl_challenges: GrandProductChallengeSet<T>,
    pub(crate) challenger_state_before: P,
    pub(crate) challenger_state_after: P,
}

impl<T: Copy + Debug + Default + Eq + PartialEq, P: PlonkyPermutation<T>> PublicInputs<T, P> {
    pub(crate) fn from_vec(v: &[T], config: &StarkConfig) -> Self {
        // TODO: Document magic number 4; probably comes from
        // Ethereum 256 bits = 4 * Goldilocks 64 bits
        let n = config.fri_config.num_cap_elements();
        let mut trace_cap = Vec::with_capacity(n);
        for i in 0..n {
            trace_cap.push(v[4 * i..4 * (i + 1)].to_vec());
        }
        let mut iter = v.iter().copied().skip(4 * n);
        let ctl_challenges = GrandProductChallengeSet {
            challenges: (0..config.num_challenges)
                .map(|_| GrandProductChallenge {
                    beta: iter.next().unwrap(),
                    gamma: iter.next().unwrap(),
                })
                .collect(),
        };
        let challenger_state_before = P::new(&mut iter);
        let challenger_state_after = P::new(&mut iter);
        let ctl_zs_last: Vec<_> = iter.collect();

        Self {
            trace_cap,
            ctl_zs_last,
            ctl_challenges,
            challenger_state_before,
            challenger_state_after,
        }
    }
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>
    RecursiveAllProof<F, C, D>
{
    /// Verify every recursive proof.
    pub fn verify(
        self,
        verifier_data: &[VerifierCircuitData<F, C, D>; NUM_TABLES],
        cross_table_lookups: Vec<CrossTableLookup<F>>,
        inner_config: &StarkConfig,
    ) -> Result<()> {
        let pis: [_; NUM_TABLES] = core::array::from_fn(|i| {
            PublicInputs::<F, <C::Hasher as Hasher<F>>::Permutation>::from_vec(
                &self.recursive_proofs[i].public_inputs,
                inner_config,
            )
        });

        let mut challenger = Challenger::<F, C::Hasher>::new();
        for pi in &pis {
            for h in &pi.trace_cap {
                challenger.observe_elements(h);
            }
        }
        let ctl_challenges =
            challenger.get_grand_product_challenge_set(inner_config.num_challenges);
        // Check that the correct CTL challenges are used in every proof.
        for pi in &pis {
            ensure!(ctl_challenges == pi.ctl_challenges);
        }

        let state = challenger.compact();
        ensure!(state == pis[0].challenger_state_before);
        // Check that the challenger state is consistent between proofs.
        for i in 1..NUM_TABLES {
            ensure!(pis[i].challenger_state_before == pis[i - 1].challenger_state_after);
        }

        // Dummy values which will make the check fail.
        // TODO: Fix this if the code isn't deprecated.
        let mut extra_looking_products = Vec::new();
        for i in 0..NUM_TABLES {
            extra_looking_products.push(Vec::new());
            for _ in 0..inner_config.num_challenges {
                extra_looking_products[i].push(F::ONE);
            }
        }

        // Verify the CTL checks.
        verify_cross_table_lookups::<F, D>(
            &cross_table_lookups,
            &pis.map(|p| p.ctl_zs_last),
            // extra_looking_products,
            inner_config,
        )?;

        // Verify the proofs.
        for (proof, verifier_data) in self.recursive_proofs.into_iter().zip(verifier_data) {
            verifier_data.verify(proof)?;
        }
        Ok(())
    }
}

/// Represents a circuit which recursively verifies a STARK proof.
#[derive(Eq, PartialEq, Debug)]
pub(crate) struct StarkWrapperCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    pub(crate) circuit: CircuitData<F, C, D>,
    pub(crate) stark_proof_target: StarkProofTarget<D>,
    pub(crate) ctl_challenges_target: GrandProductChallengeSet<Target>,
    pub(crate) init_challenger_state_target:
        <C::Hasher as AlgebraicHasher<F>>::AlgebraicPermutation,
    pub(crate) zero_target: Target,
}

impl<F, C, const D: usize> StarkWrapperCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>,
{
    pub(crate) fn prove(
        &self,
        proof_with_metadata: &StarkProofWithMetadata<F, C, D>,
        ctl_challenges: &GrandProductChallengeSet<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();

        set_stark_proof_target(
            &mut inputs,
            &self.stark_proof_target,
            &proof_with_metadata.proof,
            self.zero_target,
        );

        for (challenge_target, challenge) in self
            .ctl_challenges_target
            .challenges
            .iter()
            .zip(&ctl_challenges.challenges)
        {
            inputs.set_target(challenge_target.beta, challenge.beta);
            inputs.set_target(challenge_target.gamma, challenge.gamma);
        }

        inputs.set_target_arr(
            self.init_challenger_state_target.as_ref(),
            proof_with_metadata.init_challenger_state.as_ref(),
        );

        self.circuit.prove(inputs)
    }
}

#[allow(clippy::similar_names)]
/// Returns the recursive Stark circuit.
pub(crate) fn recursive_stark_circuit<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    table: TableKind,
    stark: &S,
    degree_bits: usize,
    cross_table_lookups: &[CrossTableLookup<F>],
    inner_config: &StarkConfig,
    circuit_config: &CircuitConfig,
    min_degree_bits: usize,
) -> StarkWrapperCircuit<F, C, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
    let zero_target = builder.zero();

    let num_permutation_zs = stark.num_permutation_batches(inner_config);
    let num_permutation_batch_size = stark.permutation_batch_size();
    let num_ctl_zs =
        CrossTableLookup::num_ctl_zs(cross_table_lookups, table, inner_config.num_challenges);
    let proof_target = add_virtual_stark_proof_with_pis(
        &mut builder,
        stark,
        inner_config,
        degree_bits,
        num_ctl_zs,
    );
    builder.register_public_inputs(
        &proof_target
            .proof
            .trace_cap
            .0
            .iter()
            .flat_map(|h| h.elements)
            .collect::<Vec<_>>(),
    );

    let ctl_challenges_target = GrandProductChallengeSet {
        challenges: (0..inner_config.num_challenges)
            .map(|_| GrandProductChallenge {
                beta: builder.add_virtual_public_input(),
                gamma: builder.add_virtual_public_input(),
            })
            .collect(),
    };

    let ctl_vars = CtlCheckVarsTarget::from_proof(
        table,
        &proof_target.proof,
        cross_table_lookups,
        &ctl_challenges_target,
        num_permutation_zs,
    );

    let init_challenger_state_target =
        <C::Hasher as AlgebraicHasher<F>>::AlgebraicPermutation::new(std::iter::from_fn(|| {
            Some(builder.add_virtual_public_input())
        }));
    let mut challenger =
        RecursiveChallenger::<F, C::Hasher, D>::from_state(init_challenger_state_target);
    let challenges = proof_target.proof.get_challenges::<F, C>(
        &mut builder,
        &mut challenger,
        num_permutation_batch_size,
        inner_config,
    );
    let challenger_state = challenger.compact(&mut builder);
    builder.register_public_inputs(challenger_state.as_ref());

    builder.register_public_inputs(&proof_target.proof.openings.ctl_zs_last);

    verify_stark_proof_with_challenges_circuit::<F, C, _, D>(
        &mut builder,
        stark,
        &proof_target,
        &challenges,
        &ctl_vars,
        inner_config,
    );

    add_common_recursion_gates(&mut builder);

    // Pad to the minimum degree.
    while log2_ceil(builder.num_gates()) < min_degree_bits {
        builder.add_gate(NoopGate, vec![]);
    }

    let circuit = builder.build::<C>();
    StarkWrapperCircuit {
        circuit,
        stark_proof_target: proof_target.proof,
        ctl_challenges_target,
        init_challenger_state_target,
        zero_target,
    }
}

/// Add gates that are sometimes used by recursive circuits, even if it's not
/// actually used by this particular recursive circuit. This is done for
/// uniformity. We sometimes want all recursion circuits to have the same gate
/// set, so that we can do 1-of-n conditional recursion efficiently.
pub(crate) fn add_common_recursion_gates<F: RichField + Extendable<D>, const D: usize>(
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
    proof: &StarkProofWithPublicInputsTarget<D>,
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
        permutation_ctl_zs,
        permutation_ctl_zs_next,
        ctl_zs_last,
        quotient_polys,
    } = &proof.proof.openings;

    let converted_public_inputs: Vec<ExtensionTarget<D>> = proof
        .public_inputs
        .iter()
        .map(|target| builder.convert_to_ext(*target)) // replace with actual conversion function/method
        .collect();

    let vars =
        S::EvaluationFrameTarget::from_values(local_values, next_values, &converted_public_inputs);

    let degree_bits = proof.proof.recover_degree_bits(inner_config);
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

    let num_permutation_zs = stark.num_permutation_batches(inner_config);
    let permutation_data = PermutationCheckDataTarget {
        local_zs: permutation_ctl_zs[..num_permutation_zs].to_vec(),
        next_zs: permutation_ctl_zs_next[..num_permutation_zs].to_vec(),
        permutation_challenge_sets: challenges.permutation_challenge_sets.clone().unwrap(),
    };

    let tmp = builder.constant(F::from_canonical_u64(14487116762836569611));
    builder.connect(
        permutation_data.permutation_challenge_sets[0].challenges[0].beta,
        tmp,
    );

    with_context!(
        builder,
        "evaluate vanishing polynomial",
        eval_vanishing_poly_circuit::<F, S, D>(
            builder,
            stark,
            inner_config,
            &vars,
            permutation_data,
            ctl_vars,
            &mut consumer,
        )
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
        // builder.connect_extension(vanishing_polys_zeta[i],
        // computed_vanishing_poly);
    }

    let merkle_caps = vec![
        proof.proof.trace_cap.clone(),
        proof.proof.permutation_ctl_zs_cap.clone(),
        proof.proof.quotient_polys_cap.clone(),
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
    // builder.verify_fri_proof::<C>(
    //     &fri_instance,
    //     &proof.proof.openings.to_fri_openings(zero),
    //     &challenges.fri_challenges,
    //     &merkle_caps,
    //     &proof.proof.opening_proof,
    //     &inner_config.fri_params(degree_bits),
    // );
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

pub(crate) fn add_virtual_stark_proof_with_pis<
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
    StarkProofWithPublicInputsTarget {
        proof,
        public_inputs,
    }
}

pub(crate) fn add_virtual_stark_proof<
    F: RichField + Extendable<D>,
    S: Stark<F, D>,
    const D: usize,
>(
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
        stark.num_permutation_batches(config) + num_ctl_zs,
        stark.quotient_degree_factor() * config.num_challenges,
    ];

    let permutation_zs_cap = builder.add_virtual_cap(cap_height);

    StarkProofTarget {
        trace_cap: builder.add_virtual_cap(cap_height),
        permutation_ctl_zs_cap: permutation_zs_cap,
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
        permutation_ctl_zs: builder
            .add_virtual_extension_targets(stark.num_permutation_batches(config) + num_ctl_zs),
        permutation_ctl_zs_next: builder
            .add_virtual_extension_targets(stark.num_permutation_batches(config) + num_ctl_zs),
        ctl_zs_last: builder.add_virtual_targets(num_ctl_zs),
        quotient_polys: builder
            .add_virtual_extension_targets(stark.quotient_degree_factor() * num_challenges),
    }
}

pub(crate) fn set_stark_proof_target<F, C: GenericConfig<D, F = F>, W, const D: usize>(
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

    witness.set_cap_target(
        &proof_target.permutation_ctl_zs_cap,
        &proof.permutation_ctl_zs_cap,
    );

    set_fri_proof_target(witness, &proof_target.opening_proof, &proof.opening_proof);
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;

    use crate::program::stark::ProgramStark;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs, TableKind};
    use crate::stark::prover::prove;
    use crate::stark::recursive_verifier::recursive_stark_circuit;
    use crate::stark::verifier::verify_proof;
    use crate::test_utils::{C, D, F};
    use crate::utils::from_u32;

    #[ignore]
    #[test]
    fn recursive_verify_program_stark() -> Result<()> {
        type S = MozakStark<F, D>;
        let stark = S::default();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 1;
        let (program, record) = simple_test_code(
            &[Instruction {
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

        let all_proof = prove::<F, C, D>(
            &program,
            &record,
            &stark,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        )?;
        verify_proof(stark.clone(), all_proof.clone(), &config)?;

        type PS = ProgramStark<F, D>;
        let mut circuit_config = CircuitConfig::standard_recursion_config();
        let degree_bits = all_proof.stark_proofs[TableKind::Program as usize]
            .proof
            .recover_degree_bits(&config);
        let stark_wrapper = recursive_stark_circuit::<F, C, PS, D>(
            TableKind::Program,
            &stark.program_stark,
            degree_bits,
            &stark.cross_table_lookups,
            &config,
            &circuit_config,
            12,
        );

        let recursive_proof = stark_wrapper.prove(
            &all_proof.stark_proofs[TableKind::Program as usize],
            &all_proof.ctl_challenges,
        )?;
        stark_wrapper.circuit.verify(recursive_proof)
    }
}
