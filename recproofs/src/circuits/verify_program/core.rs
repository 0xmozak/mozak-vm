use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use crate::circuits::build_event_root;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CircuitPublicIndices {
    /// The indices of each of the elements of the program hash
    pub program_hash: [usize; 4],

    /// The index of the presence flag for the event root
    pub events_present: usize,

    /// The indices of each of the elements of event root
    pub event_root: [usize; NUM_HASH_OUT_ELTS],

    /// The indices of each of the elements of cast root
    pub cast_root: [usize; NUM_HASH_OUT_ELTS],
}

impl CircuitPublicIndices {
    /// Extract `program_hash` from an array of public inputs.
    pub fn get_program_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; 4] {
        self.program_hash.map(|i| public_inputs[i])
    }

    /// Insert `program_hash` into an array of public inputs.
    pub fn set_program_hash<T>(&self, public_inputs: &mut [T], v: [T; 4]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.program_hash[i]] = v;
        }
    }

    pub fn get_events_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.events_present]
    }

    pub fn set_events_present<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.events_present] = v;
    }

    /// Extract `event_root` from an array of public inputs.
    pub fn get_event_root<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.event_root.map(|i| public_inputs[i])
    }

    /// Insert `event_root` into an array of public inputs.
    pub fn set_event_root<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.event_root[i]] = v;
        }
    }

    /// Extract `cast_root` from an array of public inputs.
    pub fn get_cast_root<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.cast_root.map(|i| public_inputs[i])
    }

    /// Insert `cast_root` into an array of public inputs.
    pub fn set_cast_root<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.cast_root[i]] = v;
        }
    }
}

pub trait Circuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    fn get_circuit_data(&self) -> &CircuitData<F, C, D>;
    fn get_indices(&self) -> CircuitPublicIndices;
}

pub struct ProgramVerifierTargets<const D: usize> {
    /// The program proof
    pub program_proof: ProofWithPublicInputsTarget<D>,

    /// The program hash
    pub program_hash: [Target; 4],

    /// The presence flag for the event root
    pub events_present: BoolTarget,

    /// The event root
    pub event_root: HashOutTarget,

    /// The cast list root
    pub cast_root: HashOutTarget,
}

pub struct ProgramVerifierSubCircuit<const D: usize> {
    pub targets: ProgramVerifierTargets<D>,
}

impl<const D: usize> ProgramVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        program_circuit: &dyn Circuit<F, C, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = program_circuit.get_circuit_data();
        let public_inputs = program_circuit.get_indices();
        let program_proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);

        builder.verify_proof::<C>(&program_proof, &verifier, &circuit.common);

        let program_hash = public_inputs.get_program_hash(&program_proof.public_inputs);
        let events_present =
            BoolTarget::new_unsafe(public_inputs.get_events_present(&program_proof.public_inputs));
        let event_root = HashOutTarget {
            elements: public_inputs.get_event_root(&program_proof.public_inputs),
        };
        let cast_root = HashOutTarget {
            elements: public_inputs.get_cast_root(&program_proof.public_inputs),
        };

        Self {
            program_proof,
            program_hash,
            events_present,
            event_root,
            cast_root,
        }
    }
}

impl<const D: usize> ProgramVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> ProgramVerifierSubCircuit<D> {
        ProgramVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> ProgramVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.program_proof, program_proof);
    }
}

pub struct EventRootVerifierTargets<const D: usize> {
    /// The event root proof
    pub event_root_proof: ProofWithPublicInputsTarget<D>,

    /// The event owner
    pub event_owner: [Target; 4],

    /// The event root (rp_hash)
    pub event_root: HashOutTarget,

    /// The event root (vm hash)
    pub vm_event_root: HashOutTarget,
}

pub struct EventRootVerifierSubCircuit<const D: usize> {
    pub targets: EventRootVerifierTargets<D>,
}

impl<const D: usize> EventRootVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        event_root_circuit: &build_event_root::BranchCircuit<F, C, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = &event_root_circuit.circuit;
        let event_root_proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);

        builder.verify_proof::<C>(&event_root_proof, &verifier, &circuit.common);

        let event_owner = event_root_circuit
            .event_owner
            .indices
            .get_values(&event_root_proof.public_inputs);
        let event_root = HashOutTarget {
            elements: event_root_circuit
                .hash
                .indices
                .get_unpruned_hash(&event_root_proof.public_inputs),
        };
        let vm_event_root = HashOutTarget {
            elements: event_root_circuit
                .vm_hash
                .indices
                .get_unpruned_hash(&event_root_proof.public_inputs),
        };

        Self {
            event_root_proof,
            event_owner,
            event_root,
            vm_event_root,
        }
    }
}

impl<const D: usize> EventRootVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> EventRootVerifierSubCircuit<D> {
        EventRootVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> EventRootVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        event_root_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.event_root_proof, event_root_proof);
    }
}
