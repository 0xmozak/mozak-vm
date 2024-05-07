use std::fmt::Debug;

use iter_fixed::IntoIteratorFixed;
use plonky2::hash::hash_types::{HashOutTarget, MerkleCapTarget, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::plonk::circuit_data::VerifierCircuitTarget;

pub trait Indices: Clone + PartialEq + Eq + Debug {
    type Target: ?Sized;
    type Get;
    type Any<T>;

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self;
    fn get(&self, public_inputs: &[Target]) -> Self::Get;
    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T>;
    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>);
}

/// The indices of a single `Target` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TargetIndex(pub usize);

impl TargetIndex {
    pub fn new(public_inputs: &[Target], target: Target) -> Self {
        Self(
            public_inputs
                .iter()
                .position(|&pi| pi == target)
                .expect("target not found"),
        )
    }

    pub fn get<T: Copy>(&self, public_inputs: &[T]) -> T { public_inputs[self.0] }

    pub fn set<T>(&self, public_inputs: &mut [T], v: T) { public_inputs[self.0] = v; }
}

impl Indices for TargetIndex {
    type Any<T> = T;
    type Get = Target;
    type Target = Target;

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, *target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}

/// The indices of a single `BoolTarget` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BoolTargetIndex(TargetIndex);

impl BoolTargetIndex {
    pub fn new(public_inputs: &[Target], target: BoolTarget) -> Self {
        Self(TargetIndex::new(public_inputs, target.target))
    }

    pub fn get(&self, public_inputs: &[Target]) -> BoolTarget {
        BoolTarget::new_unsafe(self.get_any(public_inputs))
    }

    pub fn get_any<T: Copy>(&self, public_inputs: &[T]) -> T { self.0.get_any(public_inputs) }

    pub fn set<T>(&self, public_inputs: &mut [T], v: T) { self.0.set(public_inputs, v); }
}

impl Indices for BoolTargetIndex {
    type Any<T> = T;
    type Get = BoolTarget;
    type Target = BoolTarget;

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, *target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get_any(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}

/// The indices of a single `HashOutTarget` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct HashTargetIndex(pub ArrayTargetIndex<TargetIndex, NUM_HASH_OUT_ELTS>);

impl HashTargetIndex {
    pub fn new(public_inputs: &[Target], target: HashOutTarget) -> Self {
        Self(ArrayTargetIndex::new(public_inputs, &target.elements))
    }

    pub fn get_any<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.0.get_any(public_inputs)
    }

    pub fn get(&self, public_inputs: &[Target]) -> HashOutTarget {
        HashOutTarget {
            elements: self.get_any(public_inputs),
        }
    }

    pub fn set<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        self.0.set(public_inputs, v)
    }
}

impl Indices for HashTargetIndex {
    type Any<T> = [T; NUM_HASH_OUT_ELTS];
    type Get = HashOutTarget;
    type Target = HashOutTarget;

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, *target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get_any(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}

/// The indices of an array of `Indicies` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ArrayTargetIndex<I, const N: usize>(pub [I; N]);

impl<I: Indices, const N: usize> ArrayTargetIndex<I, N>
where
    I::Target: Sized,
{
    pub fn new(public_inputs: &[Target], target: &[I::Target; N]) -> Self {
        Self(target.each_ref().map(|t| I::new(public_inputs, t)))
    }

    pub fn get_any<T: Copy>(&self, public_inputs: &[T]) -> [I::Any<T>; N] {
        self.0.each_ref().map(|i| i.get_any(public_inputs))
    }

    pub fn get(&self, public_inputs: &[Target]) -> [I::Get; N] {
        self.0.each_ref().map(|i| i.get(public_inputs))
    }

    pub fn set<T>(&self, public_inputs: &mut [T], v: [I::Any<T>; N]) {
        for (i, v) in self.0.each_ref().into_iter_fixed().zip(v) {
            i.set(public_inputs, v);
        }
    }
}

impl<I: Indices, const N: usize> Indices for ArrayTargetIndex<I, N>
where
    I::Target: Sized,
{
    type Any<T> = [I::Any<T>; N];
    type Get = [I::Get; N];
    type Target = [I::Target; N];

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get_any(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}

/// The indices of a `Vec` of `Indicies` in a `ProofWithPublicInputs`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VecTargetIndex<I>(pub Vec<I>);

impl<I: Indices> VecTargetIndex<I>
where
    I::Target: Sized,
{
    pub fn new(public_inputs: &[Target], target: &[I::Target]) -> Self {
        Self(target.iter().map(|t| I::new(public_inputs, t)).collect())
    }

    pub fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Vec<I::Any<T>> {
        self.0.iter().map(|i| i.get_any(public_inputs)).collect()
    }

    pub fn get(&self, public_inputs: &[Target]) -> Vec<I::Get> {
        self.0.iter().map(|i| i.get(public_inputs)).collect()
    }

    pub fn set<T>(&self, public_inputs: &mut [T], v: Vec<I::Any<T>>) {
        for (i, v) in self.0.iter().zip(v) {
            i.set(public_inputs, v)
        }
    }
}

impl<I: Indices> Indices for VecTargetIndex<I>
where
    I::Target: Sized,
{
    type Any<T> = Vec<I::Any<T>>;
    type Get = Vec<I::Get>;
    type Target = [I::Target];

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get_any(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}

/// The indices of a `MerkleCapTarget` in a `ProofWithPublicInputs`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MerkleCapTargetIndex(pub VecTargetIndex<HashTargetIndex>);

impl MerkleCapTargetIndex {
    pub fn new(public_inputs: &[Target], target: &MerkleCapTarget) -> Self {
        Self(VecTargetIndex::new(public_inputs, &target.0))
    }

    pub fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Vec<[T; NUM_HASH_OUT_ELTS]> {
        self.0.get_any(public_inputs)
    }

    pub fn get(&self, public_inputs: &[Target]) -> MerkleCapTarget {
        MerkleCapTarget(self.0.get(public_inputs))
    }

    pub fn set<T>(&self, public_inputs: &mut [T], v: Vec<[T; NUM_HASH_OUT_ELTS]>) {
        self.0.set(public_inputs, v)
    }
}

impl Indices for MerkleCapTargetIndex {
    type Any<T> = Vec<[T; NUM_HASH_OUT_ELTS]>;
    type Get = MerkleCapTarget;
    type Target = MerkleCapTarget;

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get_any(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}

#[derive(Clone, PartialEq, Eq, Debug)]
/// The indices of a `VerifierCircuitTargetIndex` in a `ProofWithPublicInputs`
pub struct VerifierCircuitTargetIndex {
    /// the indices for the digest of the "circuit" (i.e. the instance, minus
    /// public inputs), which can be used to seed Fiat-Shamir.
    pub circuit_digest: HashTargetIndex,
    /// the indices for a commitment to each constant polynomial and each
    /// permutation polynomial.
    pub constants_sigmas_cap: MerkleCapTargetIndex,
}

impl VerifierCircuitTargetIndex {
    pub fn new(public_inputs: &[Target], target: &VerifierCircuitTarget) -> Self {
        Self {
            circuit_digest: HashTargetIndex::new(public_inputs, target.circuit_digest),
            constants_sigmas_cap: MerkleCapTargetIndex::new(
                public_inputs,
                &target.constants_sigmas_cap,
            ),
        }
    }

    pub fn get_any<T: Copy>(
        &self,
        public_inputs: &[T],
    ) -> ([T; NUM_HASH_OUT_ELTS], Vec<[T; NUM_HASH_OUT_ELTS]>) {
        (
            self.circuit_digest.get_any(public_inputs),
            self.constants_sigmas_cap.get_any(public_inputs),
        )
    }

    pub fn get(&self, public_inputs: &[Target]) -> VerifierCircuitTarget {
        VerifierCircuitTarget {
            circuit_digest: self.circuit_digest.get(public_inputs),
            constants_sigmas_cap: self.constants_sigmas_cap.get(public_inputs),
        }
    }

    pub fn set<T>(
        &self,
        public_inputs: &mut [T],
        v: ([T; NUM_HASH_OUT_ELTS], Vec<[T; NUM_HASH_OUT_ELTS]>),
    ) {
        self.circuit_digest.set(public_inputs, v.0);
        self.constants_sigmas_cap.set(public_inputs, v.1);
    }
}

impl Indices for VerifierCircuitTargetIndex {
    type Any<T> = ([T; NUM_HASH_OUT_ELTS], Vec<[T; NUM_HASH_OUT_ELTS]>);
    type Get = VerifierCircuitTarget;
    type Target = VerifierCircuitTarget;

    fn new(public_inputs: &[Target], target: &Self::Target) -> Self {
        Self::new(public_inputs, target)
    }

    fn get(&self, public_inputs: &[Target]) -> Self::Get { self.get(public_inputs) }

    fn get_any<T: Copy>(&self, public_inputs: &[T]) -> Self::Any<T> { self.get_any(public_inputs) }

    fn set<T>(&self, public_inputs: &mut [T], v: Self::Any<T>) { self.set(public_inputs, v) }
}
