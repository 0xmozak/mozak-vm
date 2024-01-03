use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

/// returns `x` if b is `true` and `ZERO` otherwise
pub fn hash_if<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    x: HashOutTarget,
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    x.elements.map(|x| builder.mul(b.target, x)).into()
}

/// returns `a == b`
pub fn hash_eq<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    a: HashOutTarget,
    b: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let r = (
        builder.is_equal(a.elements[0], b.elements[0]),
        builder.is_equal(a.elements[1], b.elements[1]),
        builder.is_equal(a.elements[2], b.elements[2]),
        builder.is_equal(a.elements[3], b.elements[3]),
    );
    let r = (builder.and(r.0, r.1), builder.and(r.2, r.3));
    builder.and(r.0, r.1)
}

pub fn hash_branch<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
    let [l0, l1, l2, l3] = left.elements;
    let [r0, r1, r2, r3] = right.elements;
    Poseidon2Hash::hash_no_pad(&[l0, l1, l2, l3, r0, r1, r2, r3])
}

pub trait Circuit {
    fn pis(&self) -> usize;
    fn get_indices(&self) -> PublicIndices;
}

#[derive(Copy, Clone)]
pub struct PublicIndices {
    pub last_updated: usize,
    pub old_hash_present: usize,
    pub old_hash: [usize; NUM_HASH_OUT_ELTS],
    pub new_hash_present: usize,
    pub new_hash: [usize; NUM_HASH_OUT_ELTS],
}

impl PublicIndices {
    pub fn get_last_updated<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.last_updated]
    }

    pub fn get_old_hash_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.old_hash_present]
    }

    pub fn get_old_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.old_hash.map(|i| public_inputs[i])
    }

    pub fn get_new_hash_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.new_hash_present]
    }

    pub fn get_new_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.new_hash.map(|i| public_inputs[i])
    }

    pub fn set_last_updated<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.last_updated] = v;
    }

    pub fn set_old_hash_present<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.old_hash_present] = v;
    }

    pub fn set_old_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.old_hash[i]] = v;
        }
    }

    pub fn set_new_hash_present<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.new_hash_present] = v;
    }

    pub fn set_new_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.new_hash[i]] = v;
        }
    }
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
    pub circuit: CircuitData<F, C, D>,
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

pub struct LeafTargets {
    /// Is the previous hash present
    pub old_hash_present: BoolTarget,

    /// The previous object state
    pub old_object: LeafObjectTargets,

    /// The previous object state
    pub old_object_hash: HashOutTarget,

    /// Is the new hash present
    pub new_hash_present: BoolTarget,

    /// The previous object state
    pub new_object: LeafObjectTargets,

    /// The previous object state
    pub new_object_hash: HashOutTarget,
}

pub struct LeafObjectTargets {
    /// The object data
    pub data: [Target; 4],

    /// The object owner (program hash)
    pub owner: HashOutTarget,

    /// The object's lifetime field
    pub lifetime: Target,

    /// The object's last updated field
    pub last_updated: Target,

    /// The hash of all the objects fields
    pub hash: HashOutTarget,
}

pub struct Object<F: RichField + Extendable<D>, const D: usize> {
    /// The object data
    pub data: [F; 4],

    /// The object owner (program hash)
    pub owner: HashOut<F>,

    /// The object's lifetime field
    pub lifetime: F,

    /// The object's last updated field
    pub last_updated: F,
}
impl<F: RichField + Extendable<D>, const D: usize> Object<F, D> {
    pub fn hash(&self) -> HashOut<F> {
        let [d0, d1, d2, d3] = self.data;
        let [o0, o1, o2, o3] = self.owner.elements;
        Poseidon2Hash::hash_no_pad(&[
            d0,
            d1,
            d2,
            d3,
            o0,
            o1,
            o2,
            o3,
            self.lifetime,
            self.last_updated,
        ])
    }
}

impl<F, C, const D: usize> LeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        fn add_object<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> LeafObjectTargets
        where
            F: RichField + Extendable<D>, {
            let data = builder.add_virtual_target_arr();
            let owner = builder.add_virtual_hash();
            let lifetime = builder.add_virtual_target();
            let last_updated = builder.add_virtual_target();

            let hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
                data.into_iter()
                    .chain(owner.elements)
                    .chain([lifetime, last_updated])
                    .collect(),
            );

            LeafObjectTargets {
                data,
                owner,
                lifetime,
                last_updated,
                hash,
            }
        }
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let true_ = builder._true();

        let old_hash_present = builder.add_virtual_bool_target_safe();
        let old_object = add_object(&mut builder);
        // If we're not retaining, just use a 0 for the hash
        let old_object_hash = hash_if(&mut builder, old_hash_present, old_object.hash);

        let new_hash_present = builder.add_virtual_bool_target_safe();
        let new_object = add_object(&mut builder);
        // If we're not retaining, just use a 0 for the hash
        let new_object_hash = hash_if(&mut builder, new_hash_present, new_object.hash);

        // Add constraint to prevent expired leaves
        {
            // This is basically `let expired = lifetime < last_updated`
            let diff = builder.sub(new_object.lifetime, new_object.last_updated);
            let expired = builder.split_le(diff, 64)[63];

            // `assert(!(retain && expired))`
            // make sure we're not trying to retain an expired leaf
            let tmp = builder.and(new_hash_present, expired);
            let tmp = builder.not(tmp);
            builder.connect(tmp.target, true_.target);
        }

        builder.register_public_input(new_object.last_updated);
        builder.register_public_input(old_hash_present.target);
        builder.register_public_inputs(&old_object_hash.elements);
        builder.register_public_input(new_hash_present.target);
        builder.register_public_inputs(&new_object_hash.elements);

        let circuit = builder.build();
        let targets = LeafTargets {
            old_hash_present,
            old_object,
            old_object_hash,
            new_hash_present,
            new_object,
            new_object_hash,
        };
        let indices = PublicIndices {
            last_updated: 0,
            old_hash_present: 1,
            old_hash: [2, 3, 4, 5],
            new_hash_present: 6,
            new_hash: [7, 8, 9, 10],
        };

        assert_eq!(
            circuit.prover_only.public_inputs[indices.last_updated],
            targets.new_object.last_updated
        );
        assert_eq!(
            circuit.prover_only.public_inputs[indices.old_hash_present],
            targets.old_hash_present.target
        );
        assert_eq!(
            circuit.prover_only.public_inputs[2..(2 + NUM_HASH_OUT_ELTS)],
            targets.old_object_hash.elements
        );
        assert_eq!(
            circuit.prover_only.public_inputs[indices.new_hash_present],
            targets.new_hash_present.target
        );
        assert_eq!(
            circuit.prover_only.public_inputs[(3 + NUM_HASH_OUT_ELTS)..],
            targets.new_object_hash.elements
        );

        Self {
            circuit,
            targets,
            indices,
        }
    }

    fn set_object(
        inputs: &mut PartialWitness<F>,
        targets: &LeafObjectTargets,
        object: &Object<F, D>,
        hash: HashOut<F>,
    ) {
        inputs.set_target_arr(&targets.data, &object.data);
        inputs.set_hash_target(targets.owner, object.owner);
        inputs.set_target(targets.lifetime, object.lifetime);
        inputs.set_target(targets.last_updated, object.last_updated);
        inputs.set_hash_target(targets.hash, hash);
    }

    pub fn prove(
        &self,
        old_object: Option<(&Object<F, D>, &HashOut<F>)>,
        new_object: Result<(&Object<F, D>, &HashOut<F>), F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        if let Some((old_object, old_hash)) = old_object {
            inputs.set_bool_target(self.targets.old_hash_present, true);
            Self::set_object(&mut inputs, &self.targets.old_object, old_object, *old_hash);
            inputs.set_hash_target(self.targets.old_object_hash, *old_hash);
        } else {
            let dummy_obj = Object {
                data: [F::ZERO; 4],
                owner: HashOut::ZERO,
                lifetime: F::ZERO,
                last_updated: F::ZERO,
            };
            inputs.set_bool_target(self.targets.old_hash_present, false);
            Self::set_object(
                &mut inputs,
                &self.targets.old_object,
                &dummy_obj,
                dummy_obj.hash(),
            );
            inputs.set_hash_target(self.targets.old_object_hash, HashOut::ZERO);
        }

        match new_object {
            Ok((new_object, new_hash)) => {
                inputs.set_bool_target(self.targets.new_hash_present, true);
                Self::set_object(&mut inputs, &self.targets.new_object, new_object, *new_hash);
                inputs.set_hash_target(self.targets.new_object_hash, *new_hash);
            }
            Err(last_updated) => {
                let dummy_obj = Object {
                    data: [F::ZERO; 4],
                    owner: HashOut::ZERO,
                    lifetime: F::ZERO,
                    last_updated,
                };
                inputs.set_target(self.targets.new_object.last_updated, last_updated);
                Self::set_object(
                    &mut inputs,
                    &self.targets.new_object,
                    &dummy_obj,
                    dummy_obj.hash(),
                );
                inputs.set_bool_target(self.targets.new_hash_present, false);
            }
        }

        self.circuit.prove(inputs)
    }
}

impl<F, C, const D: usize> Circuit for LeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    fn pis(&self) -> usize { 11 }

    // TODO: don't hardcode
    fn get_indices(&self) -> PublicIndices { self.indices }
}

pub struct BranchCircuit<'a, F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
    pub circuit: CircuitData<F, C, D>,
    pub targets: BranchTargets<D>,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0`` being the lowest branch)
    /// Used for debugging
    pub height: usize,
    pub inner_circuit: &'a dyn Circuit,
}

pub struct BranchTargets<const D: usize> {
    /// The left dir
    pub left_dir: BranchDirTargets<D>,

    /// The right dir
    pub right_dir: BranchDirTargets<D>,

    /// Is the previous hash present
    pub old_hash_present: BoolTarget,

    /// The previous hash or ZERO if previously absent
    pub old_hash: HashOutTarget,

    /// Is the new hash present
    pub new_hash_present: BoolTarget,

    /// The new hash or ZERO if removed
    pub new_hash: HashOutTarget,
}
pub struct BranchDirTargets<const D: usize> {
    /// The last updated field of all objects updated in this branch
    pub last_updated: Target,

    /// Is the previous hash absent
    pub old_hash_present: BoolTarget,

    /// The previous hash or ZERO if previously absent
    pub old_hash: HashOutTarget,

    /// Is the new hash absent
    pub new_hash_present: BoolTarget,

    /// The new hash or ZERO if removed
    pub new_hash: HashOutTarget,

    /// The proof of this branch
    pub proof: ProofWithPublicInputsTarget<D>,
}

impl<'a, F, C, const D: usize> BranchCircuit<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: 'static + GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    fn from_dirs(
        inner_circuit: &'a dyn Circuit,
        mut builder: CircuitBuilder<F, D>,
        left_dir: BranchDirTargets<D>,
        right_dir: BranchDirTargets<D>,
        height: usize,
    ) -> Self {
        builder.connect(left_dir.last_updated, right_dir.last_updated);

        let old_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            left_dir
                .old_hash
                .elements
                .into_iter()
                .chain(right_dir.old_hash.elements)
                .collect(),
        );
        let new_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            left_dir
                .new_hash
                .elements
                .into_iter()
                .chain(right_dir.new_hash.elements)
                .collect(),
        );

        let old_hash_present = builder.and(left_dir.new_hash_present, right_dir.old_hash_present);
        let new_hash_present = builder.and(left_dir.new_hash_present, right_dir.new_hash_present);

        let old_hash = hash_if(&mut builder, old_hash_present, old_hash);
        let new_hash = hash_if(&mut builder, new_hash_present, new_hash);

        builder.register_public_input(left_dir.last_updated);
        builder.register_public_input(old_hash_present.target);
        builder.register_public_inputs(&old_hash.elements);
        builder.register_public_input(new_hash_present.target);
        builder.register_public_inputs(&new_hash.elements);

        let circuit = builder.build();
        let targets = BranchTargets {
            left_dir,
            right_dir,
            old_hash_present,
            old_hash,
            new_hash_present,
            new_hash,
        };
        let indices = PublicIndices {
            last_updated: 0,
            old_hash_present: 1,
            old_hash: [2, 3, 4, 5],
            new_hash_present: 6,
            new_hash: [7, 8, 9, 10],
        };

        assert_eq!(
            circuit.prover_only.public_inputs[indices.last_updated],
            targets.left_dir.last_updated
        );
        assert_eq!(
            circuit.prover_only.public_inputs[indices.old_hash_present],
            targets.old_hash_present.target
        );
        assert_eq!(
            circuit.prover_only.public_inputs[2..(2 + NUM_HASH_OUT_ELTS)],
            targets.old_hash.elements
        );
        assert_eq!(
            circuit.prover_only.public_inputs[indices.new_hash_present],
            targets.new_hash_present.target
        );
        assert_eq!(
            circuit.prover_only.public_inputs[(3 + NUM_HASH_OUT_ELTS)..],
            targets.new_hash.elements
        );

        Self {
            circuit,
            targets,
            indices,
            height,
            inner_circuit,
        }
    }

    fn dir_from_leaf(
        builder: &mut CircuitBuilder<F, D>,
        leaf: &LeafCircuit<F, C, D>,
    ) -> BranchDirTargets<D> {
        let verifier = builder.constant_verifier_data(&leaf.circuit.verifier_only);
        let proof = builder.add_virtual_proof_with_pis(&leaf.circuit.common);
        let leaf_idx = leaf.get_indices();

        let old_hash_present =
            BoolTarget::new_unsafe(leaf_idx.get_old_hash_present(&proof.public_inputs));
        let new_hash_present =
            BoolTarget::new_unsafe(leaf_idx.get_new_hash_present(&proof.public_inputs));

        // Ensure both hashes are not absent
        let either_present = builder.or(old_hash_present, new_hash_present);
        builder.assert_bool(either_present);

        let old_hash = HashOutTarget::from(leaf_idx.get_old_hash(&proof.public_inputs));
        let new_hash = HashOutTarget::from(leaf_idx.get_new_hash(&proof.public_inputs));

        let is_not_updated = hash_eq(builder, old_hash, new_hash);
        let is_updated = builder.not(is_not_updated);

        builder
            .conditionally_verify_proof_or_dummy::<C>(
                is_updated,
                &proof,
                &verifier,
                &leaf.circuit.common,
            )
            .expect("failed to build recursive proof verifier");

        BranchDirTargets {
            last_updated: leaf_idx.get_last_updated(&proof.public_inputs),
            old_hash_present,
            old_hash,
            new_hash_present,
            new_hash,
            proof,
        }
    }

    pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &'a LeafCircuit<F, C, D>) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let left_dir = Self::dir_from_leaf(&mut builder, leaf);
        let right_dir = Self::dir_from_leaf(&mut builder, leaf);
        Self::from_dirs(leaf, builder, left_dir, right_dir, 0)
    }

    fn dir_from_branch(builder: &mut CircuitBuilder<F, D>, branch: &Self) -> BranchDirTargets<D> {
        let verifier = builder.constant_verifier_data(&branch.circuit.verifier_only);
        let proof = builder.add_virtual_proof_with_pis(&branch.circuit.common);

        let old_hash_present =
            BoolTarget::new_unsafe(branch.get_old_hash_present(&proof.public_inputs));
        let new_hash_present =
            BoolTarget::new_unsafe(branch.get_new_hash_present(&proof.public_inputs));

        // Ensure both hashes are not absent
        let either_present = builder.or(old_hash_present, new_hash_present);
        builder.assert_bool(either_present);

        let old_hash = HashOutTarget::from(branch.get_old_hash(&proof.public_inputs));
        let new_hash = HashOutTarget::from(branch.get_new_hash(&proof.public_inputs));

        let is_not_updated = hash_eq(builder, old_hash, new_hash);
        let is_updated = builder.not(is_not_updated);

        builder
            .conditionally_verify_proof_or_dummy::<C>(
                is_updated,
                &proof,
                &verifier,
                &branch.circuit.common,
            )
            .expect("failed to build recursive proof verifier");

        BranchDirTargets {
            last_updated: branch.get_last_updated(&proof.public_inputs),
            old_hash_present,
            old_hash,
            new_hash_present,
            new_hash,
            proof,
        }
    }

    pub fn from_branch(circuit_config: &CircuitConfig, branch: &'a Self) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let left_dir = Self::dir_from_branch(&mut builder, branch);
        let right_dir = Self::dir_from_branch(&mut builder, branch);

        builder.connect(left_dir.last_updated, right_dir.last_updated);
        Self::from_dirs(branch, builder, left_dir, right_dir, branch.height + 1)
    }

    pub fn prove(
        &self,
        left_old_hash: Option<&HashOut<F>>,
        left_new_proof: Option<&ProofWithPublicInputs<F, C, D>>,
        right_old_hash: Option<&HashOut<F>>,
        right_new_proof: Option<&ProofWithPublicInputs<F, C, D>>,
        old_hash: Option<&HashOut<F>>,
        new_hash: Option<&HashOut<F>>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();

        let pub_idx = self.inner_circuit.get_indices();

        let (left_old_hash_present, left_old_hash) = if let Some(old_hash) = left_old_hash {
            (true, *old_hash)
        } else {
            (false, HashOut::ZERO)
        };
        inputs.set_bool_target(
            self.targets.left_dir.old_hash_present,
            left_old_hash_present,
        );
        inputs.set_hash_target(self.targets.left_dir.old_hash, left_old_hash);

        let (right_old_hash_present, right_old_hash) = if let Some(old_hash) = right_old_hash {
            (true, *old_hash)
        } else {
            (false, HashOut::ZERO)
        };
        inputs.set_bool_target(
            self.targets.right_dir.old_hash_present,
            right_old_hash_present,
        );
        inputs.set_hash_target(self.targets.right_dir.old_hash, right_old_hash);

        let (old_hash_present, old_hash) = if let Some(old_hash) = old_hash {
            (true, *old_hash)
        } else {
            (false, HashOut::ZERO)
        };
        inputs.set_bool_target(self.targets.old_hash_present, old_hash_present);
        inputs.set_hash_target(self.targets.old_hash, old_hash);

        let (new_hash_present, new_hash) = if let Some(new_hash) = new_hash {
            (true, *new_hash)
        } else {
            (false, HashOut::ZERO)
        };
        inputs.set_bool_target(self.targets.new_hash_present, new_hash_present);
        inputs.set_hash_target(self.targets.new_hash, new_hash);

        match (left_new_proof, right_new_proof) {
            (None, None) => panic!("can't be missing both sides"),
            (Some(left_new_proof), Some(right_new_proof)) => {
                inputs.set_proof_with_pis_target(&self.targets.left_dir.proof, left_new_proof);
                inputs.set_proof_with_pis_target(&self.targets.right_dir.proof, right_new_proof);
            }
            (None, Some(right_new_proof)) => {
                let mut left_new_proof = right_new_proof.clone();
                let public_inputs = &mut left_new_proof.public_inputs;
                pub_idx.set_old_hash_present(public_inputs, F::from_bool(left_old_hash_present));
                pub_idx.set_old_hash(public_inputs, left_old_hash.elements);
                pub_idx.set_new_hash_present(public_inputs, F::from_bool(left_old_hash_present));
                pub_idx.set_new_hash(public_inputs, left_old_hash.elements);

                inputs.set_proof_with_pis_target(&self.targets.left_dir.proof, &left_new_proof);
                inputs.set_proof_with_pis_target(&self.targets.right_dir.proof, right_new_proof);
            }
            (Some(left_new_proof), None) => {
                let mut right_new_proof = left_new_proof.clone();
                let public_inputs = &mut right_new_proof.public_inputs;
                pub_idx.set_old_hash_present(public_inputs, F::from_bool(right_old_hash_present));
                pub_idx.set_old_hash(public_inputs, right_old_hash.elements);
                pub_idx.set_new_hash_present(public_inputs, F::from_bool(right_old_hash_present));
                pub_idx.set_new_hash(public_inputs, right_old_hash.elements);

                inputs.set_proof_with_pis_target(&self.targets.left_dir.proof, left_new_proof);
                inputs.set_proof_with_pis_target(&self.targets.right_dir.proof, &right_new_proof);
            }
        }

        self.circuit.prove(inputs)
    }

    pub fn get_last_updated<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.indices.last_updated]
    }

    pub fn get_old_hash_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.indices.old_hash_present]
    }

    pub fn get_old_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.indices.old_hash.map(|i| public_inputs[i])
    }

    pub fn get_new_hash_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.indices.new_hash_present]
    }

    pub fn get_new_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.indices.new_hash.map(|i| public_inputs[i])
    }
}

impl<'a, F, C, const D: usize> Circuit for BranchCircuit<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    fn pis(&self) -> usize { 11 }

    // TODO: don't hardcode
    fn get_indices(&self) -> PublicIndices { self.indices }
}

#[cfg(test)]
mod test {
    use plonky2::field::types::{Field, Sample};
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::*;
    use crate::test_utils::{C, D, F};

    fn hash_str(v: &str) -> HashOut<F> {
        let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
        Poseidon2Hash::hash_no_pad(&v)
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let data_v1 = F::rand_array();
        let data_v2 = F::rand_array();

        let owner = hash_str("Totally A Program");

        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let object_v1 = Object::<F, D> {
            data: data_v1,
            last_updated: F::from_canonical_u8(42),
            lifetime: F::from_canonical_u8(69),
            owner,
        };
        let object_v2 = Object {
            data: data_v2,
            last_updated: F::from_canonical_u8(43),
            lifetime: F::from_canonical_u8(69),
            owner,
        };

        let object_v1_pair = (&object_v1, &object_v1.hash());
        let object_v2_pair = (&object_v2, &object_v2.hash());

        // Create an object
        let proof = circuit.prove(None, Ok(object_v1_pair))?;
        circuit.circuit.verify(proof)?;

        // Update an object
        let proof = circuit.prove(Some(object_v1_pair), Ok(object_v2_pair))?;
        circuit.circuit.verify(proof)?;

        // Delete an object
        let proof = circuit.prove(Some(object_v1_pair), Err(object_v2.last_updated))?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn expired_leaf() {
        let data = F::rand_array();

        let owner_fields: Vec<_> = "Totally A Program"
            .bytes()
            .map(F::from_canonical_u8)
            .collect();
        let owner = Poseidon2Hash::hash_no_pad(&owner_fields);

        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let object_v1 = Object::<F, D> {
            data,
            last_updated: F::from_canonical_u8(42),
            lifetime: F::from_canonical_u8(69),
            owner,
        };
        let object_v2 = Object {
            data,
            last_updated: F::from_canonical_u8(70),
            lifetime: F::from_canonical_u8(69),
            owner,
        };

        let object_v1_pair = (&object_v1, &object_v1.hash());
        let object_v2_pair = (&object_v2, &object_v2.hash());

        // Update an object
        let proof = circuit.prove(Some(object_v1_pair), Ok(object_v2_pair));
        assert!(proof.is_err());
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let data_a_v1 = F::rand_array();
        let data_a_v2 = F::rand_array();
        let data_b = F::rand_array();

        let owner_a = hash_str("Totally A Program");
        let owner_b = hash_str("A Different Program");

        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let object_a_v1 = Object::<F, D> {
            data: data_a_v1,
            last_updated: F::from_canonical_u8(42),
            lifetime: F::from_canonical_u8(69),
            owner: owner_a,
        };
        let object_a_v2 = Object::<F, D> {
            data: data_a_v2,
            last_updated: F::from_canonical_u8(44),
            lifetime: F::from_canonical_u8(69),
            owner: owner_a,
        };
        let object_a_v1_pair = (&object_a_v1, &object_a_v1.hash());
        let object_a_v2_pair = (&object_a_v2, &object_a_v2.hash());

        // Update an object
        let leaf_proof = leaf_circuit.prove(Some(object_a_v1_pair), Ok(object_a_v2_pair))?;
        leaf_circuit.circuit.verify(leaf_proof.clone())?;

        // No updates
        let object_b = Object::<F, D> {
            data: data_b,
            last_updated: F::from_canonical_u8(43),
            lifetime: F::from_canonical_u8(99),
            owner: owner_b,
        };
        let object_b_pair = (&object_b, &object_b.hash());

        // Branch
        let branch_circuit = BranchCircuit::<F, C, D>::from_leaf(&circuit_config, &leaf_circuit);

        let branch_ab = hash_branch(object_a_v1_pair.1, object_b_pair.1);
        let branch_ab_v2 = hash_branch(object_a_v2_pair.1, object_b_pair.1);

        let branch_proof = branch_circuit.prove(
            Some(object_a_v1_pair.1),
            Some(&leaf_proof),
            Some(object_b_pair.1),
            None,
            Some(&branch_ab),
            Some(&branch_ab_v2),
        )?;
        branch_circuit.circuit.verify(branch_proof)?;

        Ok(())
    }
}
