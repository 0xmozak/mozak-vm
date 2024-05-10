use std::time::Duration;

use anyhow::Result;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mozak_recproofs::circuits::state_update::{BranchCircuit, LeafCircuit};
use mozak_recproofs::subcircuits::{propagate, unbounded};
use mozak_recproofs::test_utils::{hash_str, C, D, F};
use plonky2::field::types::Field;
use plonky2::hash::hash_types::HashOut;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::proof::ProofWithPublicInputs;

pub struct DummyLeafCircuit {
    pub unbounded: unbounded::LeafSubCircuit,
    pub propagate: propagate::LeafSubCircuit<4>,
    pub circuit: CircuitData<F, C, D>,
}

impl DummyLeafCircuit {
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let propagate_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let propagate_targets = propagate_inputs.build_leaf(&mut builder);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let propagate = propagate_targets.build(public_inputs);

        Self {
            propagate,
            unbounded,
            circuit,
        }
    }

    pub fn prove(
        &self,
        leaf_value: HashOut<F>,
        branch: &DummyBranchCircuit,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.propagate.set_witness(&mut inputs, leaf_value.elements);
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.circuit.prove(inputs)
    }
}

pub struct DummyBranchCircuit {
    pub unbounded: unbounded::BranchSubCircuit<D>,
    pub propagate: propagate::BranchSubCircuit<4>,
    pub circuit: CircuitData<F, C, D>,
}

impl DummyBranchCircuit {
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let propagate_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let propagate_targets = propagate_inputs.build_branch(
            &mut builder,
            &leaf.propagate.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let propagate = propagate_targets.build(&leaf.propagate.indices, public_inputs);

        Self {
            propagate,
            unbounded,
            circuit,
        }
    }

    pub fn prove(
        &self,
        left_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_is_leaf: bool,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(
            &mut inputs,
            left_is_leaf,
            left_proof,
            right_is_leaf,
            right_proof,
        );
        self.circuit.prove(inputs)
    }
}

fn bench_prove_verify_recproof(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove_verify_recproof");
    group.measurement_time(Duration::new(10, 0));

    let circuit_config = CircuitConfig::standard_recursion_config();
    let leaf_circuit = black_box(LeafCircuit::<F, C, D>::new(&circuit_config));
    let branch_circuit_1 = BranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
    let branch_circuit_2 = BranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

    let zero_hash = black_box(HashOut::from([F::ZERO; 4]));
    let non_zero_hash_1 = black_box(hash_str("Non-Zero Hash 1"));

    // Leaf proofs
    let zero_proof = leaf_circuit.prove(zero_hash, zero_hash, None).unwrap();
    leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

    let proof_0_to_1_id_3 = leaf_circuit
        .prove(zero_hash, non_zero_hash_1, Some(3))
        .unwrap();
    leaf_circuit
        .circuit
        .verify(proof_0_to_1_id_3.clone())
        .unwrap();

    let proof_0_to_1_id_4 = leaf_circuit
        .prove(zero_hash, non_zero_hash_1, Some(4))
        .unwrap();
    leaf_circuit
        .circuit
        .verify(proof_0_to_1_id_4.clone())
        .unwrap();

    // Branch proofs
    let branch_00_and_01_proof = branch_circuit_1
        .prove(&zero_proof, &proof_0_to_1_id_3)
        .unwrap();

    let branch_01_and_00_proof = branch_circuit_1
        .prove(&proof_0_to_1_id_4, &zero_proof)
        .unwrap();

    // Benches
    group.bench_function("recproof_leaf_prove", |b| {
        b.iter(|| {
            leaf_circuit
                .prove(zero_hash, non_zero_hash_1, Some(3))
                .unwrap()
        })
    });
    group.bench_function("recproof_leaf_verify", |b| {
        b.iter(|| {
            leaf_circuit
                .circuit
                .verify(proof_0_to_1_id_3.clone())
                .unwrap()
        })
    });

    group.bench_function("recproof_branch_prove_1", |b| {
        b.iter(|| {
            branch_circuit_1
                .prove(&zero_proof, &proof_0_to_1_id_3)
                .unwrap()
        })
    });
    group.bench_function("recproof_branch_verify", |b| {
        b.iter(|| {
            branch_circuit_1
                .circuit
                .verify(branch_00_and_01_proof.clone())
                .unwrap()
        })
    });

    group.bench_function("recproof_branch_prove_2", |b| {
        b.iter(|| {
            branch_circuit_2
                .prove(&branch_00_and_01_proof, &branch_01_and_00_proof)
                .unwrap()
        })
    });

    group.finish();
}

fn bench_prove_verify_unbounded(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove_verify_unbounded");
    group.measurement_time(Duration::new(10, 0));

    let circuit_config = CircuitConfig::standard_recursion_config();
    let leaf = black_box(DummyLeafCircuit::new(&circuit_config));
    let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

    let non_zero_hash = black_box(hash_str("Non-Zero Hash"));

    let leaf_1_proof = leaf.prove(non_zero_hash, &branch).unwrap();
    leaf.circuit.verify(leaf_1_proof.clone()).unwrap();

    let leaf_2_proof = leaf.prove(non_zero_hash, &branch).unwrap();
    leaf.circuit.verify(leaf_2_proof.clone()).unwrap();

    let branch_proof_1 = branch
        .prove(true, &leaf_1_proof, true, &leaf_2_proof)
        .unwrap();
    branch.circuit.verify(branch_proof_1.clone()).unwrap();

    let branch_proof_2 = branch
        .prove(true, &leaf_2_proof, true, &leaf_2_proof)
        .unwrap();
    branch.circuit.verify(branch_proof_2.clone()).unwrap();

    let double_branch_proof = branch
        .prove(true, &leaf_2_proof, false, &branch_proof_2)
        .unwrap();
    branch.circuit.verify(double_branch_proof.clone()).unwrap();

    group.bench_function("branch_prove_1", |b| {
        b.iter(|| {
            branch
                .prove(true, &leaf_1_proof, true, &leaf_2_proof)
                .unwrap()
        })
    });
    group.bench_function("branch_verify_1", |b| {
        b.iter(|| branch.circuit.verify(branch_proof_1.clone()).unwrap())
    });

    group.bench_function("branch_prove_2", |b| {
        b.iter(|| {
            branch
                .prove(true, &leaf_2_proof, false, &branch_proof_2)
                .unwrap()
        })
    });
    group.bench_function("branch_verify_2", |b| {
        b.iter(|| branch.circuit.verify(double_branch_proof.clone()).unwrap())
    });

    group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_prove_verify_recproof, bench_prove_verify_unbounded
];
criterion_main!(benches);
