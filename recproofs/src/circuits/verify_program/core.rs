use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CircuitPublicIndices {
    /// The indices of each of the elements of the program hash
    pub program_hash: [usize; 4],

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

    /// Extract `cast_root` from an array of public inputs.
    pub fn get_cast_root<T: Copy>(&self, public_inputs: &[T]) -> [T; 4] {
        self.cast_root.map(|i| public_inputs[i])
    }

    /// Insert `cast_root` into an array of public inputs.
    pub fn set_cast_root<T>(&self, public_inputs: &mut [T], v: [T; 4]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.cast_root[i]] = v;
        }
    }
}

pub trait Circuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    fn get_circuit_data(&self) -> CircuitData<F, C, D>;
    fn get_indices(&self) -> CircuitPublicIndices;
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices;

pub struct SubCircuitInputs;

pub struct LeafTargets<const D: usize> {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The program proof
    pub program_proof: ProofWithPublicInputsTarget<D>,

    /// The program hash
    pub program_hash: [Target; 4],

    /// The value to be propagated throughout the produced tree
    pub cast_root: HashOutTarget,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(_builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        Self {}
    }

    #[must_use]
    pub fn build_leaf<F, C, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        program: &impl Circuit<F, C, D>,
    ) -> LeafTargets<D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = program.get_circuit_data();
        let public_inputs = program.get_indices();
        let program_proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);

        builder.verify_proof::<C>(&program_proof, &verifier, &circuit.common);

        let program_hash = public_inputs.get_program_hash(&program_proof.public_inputs);
        let cast_root = HashOutTarget {
            elements: public_inputs.get_cast_root(&program_proof.public_inputs),
        };

        LeafTargets {
            inputs: self,
            program_proof,
            program_hash,
            cast_root,
        }
    }
}

pub struct LeafSubCircuit<const D: usize> {
    pub targets: LeafTargets<D>,
    pub indices: PublicIndices,
}

impl<const D: usize> LeafTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> LeafSubCircuit<D> {
        let indices = PublicIndices;
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl<const D: usize> LeafSubCircuit<D> {
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

pub struct BranchTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,
}

impl SubCircuitInputs {
    fn direction_from_node<const D: usize>(
        _proof: &ProofWithPublicInputsTarget<D>,
        _indices: &PublicIndices,
    ) -> SubCircuitInputs {
        SubCircuitInputs
    }

    #[must_use]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        _builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, indices);
        let right = Self::direction_from_node(right_proof, indices);

        BranchTargets {
            inputs: self,
            left,
            right,
        }
    }
}

pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

impl BranchTargets {
    #[must_use]
    pub fn build(self, child: &PublicIndices, _public_inputs: &[Target]) -> BranchSubCircuit {
        let indices = PublicIndices;
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(&self, _inputs: &mut PartialWitness<F>) {}
}
