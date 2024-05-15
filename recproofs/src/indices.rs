use std::fmt::Debug;
use std::marker::PhantomData;

use iter_fixed::IntoIteratorFixed;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::{
    HashOut, HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS,
};
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::plonk::circuit_data::{VerifierCircuitTarget, VerifierOnlyCircuitData};
use plonky2::plonk::config::{GenericConfig, Hasher};

pub trait Indices: Clone + PartialEq + Eq + Debug {
    type SourceTarget: ?Sized;
    type Target;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self;
    fn get_target(&self, public_inputs: &[Target]) -> Self::Target;
    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target);
}

pub struct ConfigMarked<C, const D: usize, T> {
    pub _marker: PhantomData<C>,
    pub t: T,
}

pub struct HashMarked<H, T> {
    pub _marker: PhantomData<H>,
    pub t: T,
}

pub trait FieldIndices<F> {
    type Field;
    fn get_field(&self, public_inputs: &[F]) -> Self::Field;
    fn set_field(&self, public_inputs: &mut [F], v: Self::Field);
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

    fn get_raw<T: Copy>(&self, public_inputs: &[T]) -> T { public_inputs[self.0] }

    pub fn get_target(&self, public_inputs: &[Target]) -> Target { self.get_raw(public_inputs) }

    pub fn get_field<F: Copy>(&self, public_inputs: &[F]) -> F { self.get_raw(public_inputs) }

    fn set_raw<T>(&self, public_inputs: &mut [T], v: T) { public_inputs[self.0] = v; }

    pub fn set_target(&self, public_inputs: &mut [Target], v: Target) {
        self.set_raw(public_inputs, v)
    }

    pub fn set_field<F>(&self, public_inputs: &mut [F], v: F) { self.set_raw(public_inputs, v) }
}

impl Indices for TargetIndex {
    type SourceTarget = Target;
    type Target = Target;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, *target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<F: Copy> FieldIndices<F> for TargetIndex {
    type Field = F;

    fn get_field(&self, public_inputs: &[F]) -> Self::Field { self.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [F], v: Self::Field) {
        self.set_field(public_inputs, v)
    }
}

/// The indices of a single `BoolTarget` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BoolTargetIndex(pub TargetIndex);

impl BoolTargetIndex {
    pub fn new(public_inputs: &[Target], target: BoolTarget) -> Self {
        Self(TargetIndex::new(public_inputs, target.target))
    }

    pub fn get_target(&self, public_inputs: &[Target]) -> BoolTarget {
        BoolTarget::new_unsafe(self.0.get_target(public_inputs))
    }

    pub fn get_field<F: Field>(&self, public_inputs: &[F]) -> bool {
        let v = self.0.get_field(public_inputs);
        debug_assert!(v.is_zero() || v.is_one());
        v.is_one()
    }

    pub fn set_target(&self, public_inputs: &mut [Target], v: BoolTarget) {
        self.0.set_target(public_inputs, v.target)
    }

    pub fn set_field<F: Field>(&self, public_inputs: &mut [F], v: bool) {
        self.0.set_field(public_inputs, Field::from_bool(v))
    }
}

impl Indices for BoolTargetIndex {
    type SourceTarget = BoolTarget;
    type Target = BoolTarget;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, *target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<F: Field> FieldIndices<F> for BoolTargetIndex {
    type Field = bool;

    fn get_field(&self, public_inputs: &[F]) -> Self::Field { self.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [F], v: Self::Field) {
        self.set_field(public_inputs, v)
    }
}

/// The indices of a single `HashOutTarget` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct HashOutTargetIndex(pub ArrayTargetIndex<TargetIndex, NUM_HASH_OUT_ELTS>);

impl HashOutTargetIndex {
    pub fn new(public_inputs: &[Target], target: HashOutTarget) -> Self {
        Self(ArrayTargetIndex::new(public_inputs, &target.elements))
    }

    pub fn get_target(&self, public_inputs: &[Target]) -> HashOutTarget {
        HashOutTarget {
            elements: self.0.get_target(public_inputs),
        }
    }

    pub fn get_field<F: Field>(&self, public_inputs: &[F]) -> HashOut<F> {
        HashOut {
            elements: self.0 .0.each_ref().map(|i| i.get_field(public_inputs)),
        }
    }

    pub fn set_target(&self, public_inputs: &mut [Target], v: HashOutTarget) {
        self.0.set_target(public_inputs, v.elements)
    }

    pub fn set_field<F: Field>(&self, public_inputs: &mut [F], v: HashOut<F>) {
        self.0.set_field(public_inputs, v.elements)
    }
}

impl Indices for HashOutTargetIndex {
    type SourceTarget = HashOutTarget;
    type Target = HashOutTarget;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, *target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<F: Field> FieldIndices<F> for HashOutTargetIndex {
    type Field = HashOut<F>;

    fn get_field(&self, public_inputs: &[F]) -> Self::Field { self.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [F], v: Self::Field) {
        self.set_field(public_inputs, v)
    }
}

/// The indices of an array of `Indicies` in a `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ArrayTargetIndex<I, const N: usize>(pub [I; N]);

impl<I, const N: usize> ArrayTargetIndex<I, N> {
    pub fn new(public_inputs: &[Target], target: &[I::SourceTarget; N]) -> Self
    where
        I: Indices,
        I::SourceTarget: Sized, {
        Self(target.each_ref().map(|t| I::new(public_inputs, t)))
    }

    pub fn get_target(&self, public_inputs: &[Target]) -> [I::Target; N]
    where
        I: Indices, {
        self.0.each_ref().map(|i| i.get_target(public_inputs))
    }

    pub fn get_field<F>(&self, public_inputs: &[F]) -> [I::Field; N]
    where
        I: FieldIndices<F>, {
        self.0.each_ref().map(|i| i.get_field(public_inputs))
    }

    pub fn set_target(&self, public_inputs: &mut [Target], v: [I::Target; N])
    where
        I: Indices, {
        for (i, v) in self.0.each_ref().into_iter_fixed().zip(v) {
            i.set_target(public_inputs, v);
        }
    }

    pub fn set_field<F>(&self, public_inputs: &mut [F], v: [I::Field; N])
    where
        I: FieldIndices<F>, {
        for (i, v) in self.0.each_ref().into_iter_fixed().zip(v) {
            i.set_field(public_inputs, v);
        }
    }
}

impl<I: Indices, const N: usize> Indices for ArrayTargetIndex<I, N>
where
    I::SourceTarget: Sized,
{
    type SourceTarget = [I::SourceTarget; N];
    type Target = [I::Target; N];

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<F: Field, I: FieldIndices<F>, const N: usize> FieldIndices<F> for ArrayTargetIndex<I, N> {
    type Field = [I::Field; N];

    fn get_field(&self, public_inputs: &[F]) -> Self::Field { self.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [F], v: Self::Field) {
        self.set_field(public_inputs, v)
    }
}

/// The indices of a `Vec` of `Indicies` in a `ProofWithPublicInputs`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VecTargetIndex<I>(pub Vec<I>);

impl<I> VecTargetIndex<I> {
    pub fn new(public_inputs: &[Target], target: &[I::SourceTarget]) -> Self
    where
        I: Indices,
        I::SourceTarget: Sized, {
        Self(target.iter().map(|t| I::new(public_inputs, t)).collect())
    }

    pub fn get_target(&self, public_inputs: &[Target]) -> Vec<I::Target>
    where
        I: Indices, {
        self.0.iter().map(|i| i.get_target(public_inputs)).collect()
    }

    pub fn get_field<F>(&self, public_inputs: &[F]) -> Vec<I::Field>
    where
        I: FieldIndices<F>, {
        self.0.iter().map(|i| i.get_field(public_inputs)).collect()
    }

    pub fn set_target(&self, public_inputs: &mut [Target], v: Vec<I::Target>)
    where
        I: Indices, {
        for (i, v) in self.0.iter().zip(v) {
            i.set_target(public_inputs, v)
        }
    }

    pub fn set_field<F>(&self, public_inputs: &mut [F], v: Vec<I::Field>)
    where
        I: FieldIndices<F>, {
        for (i, v) in self.0.iter().zip(v) {
            i.set_field(public_inputs, v)
        }
    }
}

impl<I: Indices> Indices for VecTargetIndex<I>
where
    I::SourceTarget: Sized,
{
    type SourceTarget = [I::SourceTarget];
    type Target = Vec<I::Target>;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<F: Field, I: FieldIndices<F>> FieldIndices<F> for VecTargetIndex<I> {
    type Field = Vec<I::Field>;

    fn get_field(&self, public_inputs: &[F]) -> Self::Field { self.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [F], v: Self::Field) {
        self.set_field(public_inputs, v)
    }
}

/// The indices of a `MerkleCapTarget` in a `ProofWithPublicInputs`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MerkleCapTargetIndex(pub VecTargetIndex<HashOutTargetIndex>);

impl MerkleCapTargetIndex {
    pub fn new(public_inputs: &[Target], target: &MerkleCapTarget) -> Self {
        Self(VecTargetIndex::new(public_inputs, &target.0))
    }

    pub fn get_target(&self, public_inputs: &[Target]) -> MerkleCapTarget {
        MerkleCapTarget(self.0.get_target(public_inputs))
    }

    pub fn get_field<F, H>(&self, public_inputs: &[F]) -> MerkleCap<F, H>
    where
        F: RichField,
        H: Hasher<F, Hash = HashOut<F>>, {
        MerkleCap(self.0.get_field(public_inputs))
    }

    pub fn set_target(&self, public_inputs: &mut [Target], v: MerkleCapTarget) {
        self.0.set_target(public_inputs, v.0)
    }

    pub fn set_field<F, H>(&self, public_inputs: &mut [F], v: MerkleCap<F, H>)
    where
        F: RichField,
        H: Hasher<F, Hash = HashOut<F>>, {
        self.0.set_field(public_inputs, v.0)
    }
}

impl Indices for MerkleCapTargetIndex {
    type SourceTarget = MerkleCapTarget;
    type Target = MerkleCapTarget;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<F, H> FieldIndices<F> for HashMarked<H, MerkleCapTargetIndex>
where
    F: RichField,
    H: Hasher<F, Hash = HashOut<F>>,
{
    type Field = MerkleCap<F, H>;

    fn get_field(&self, public_inputs: &[F]) -> Self::Field { self.t.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [F], v: Self::Field) {
        self.t.set_field(public_inputs, v)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
/// The indices of a `VerifierCircuitTargetIndex` in a `ProofWithPublicInputs`
pub struct VerifierCircuitTargetIndex {
    /// the indices for a commitment to each constant polynomial and each
    /// permutation polynomial.
    pub constants_sigmas_cap: MerkleCapTargetIndex,
    /// the indices for the digest of the "circuit" (i.e. the instance, minus
    /// public inputs), which can be used to seed Fiat-Shamir.
    pub circuit_digest: HashOutTargetIndex,
}

impl VerifierCircuitTargetIndex {
    pub fn new(public_inputs: &[Target], target: &VerifierCircuitTarget) -> Self {
        Self {
            constants_sigmas_cap: MerkleCapTargetIndex::new(
                public_inputs,
                &target.constants_sigmas_cap,
            ),
            circuit_digest: HashOutTargetIndex::new(public_inputs, target.circuit_digest),
        }
    }

    pub fn get_target(&self, public_inputs: &[Target]) -> VerifierCircuitTarget {
        VerifierCircuitTarget {
            constants_sigmas_cap: self.constants_sigmas_cap.get_target(public_inputs),
            circuit_digest: self.circuit_digest.get_target(public_inputs),
        }
    }

    pub fn get_field<C, const D: usize>(
        &self,
        public_inputs: &[C::F],
    ) -> VerifierOnlyCircuitData<C, D>
    where
        C: GenericConfig<D>,
        C::Hasher: Hasher<C::F, Hash = HashOut<C::F>>, {
        VerifierOnlyCircuitData {
            constants_sigmas_cap: self.constants_sigmas_cap.get_field(public_inputs),
            circuit_digest: self.circuit_digest.get_field(public_inputs),
        }
    }

    pub fn set_target(&self, public_inputs: &mut [Target], v: VerifierCircuitTarget) {
        self.constants_sigmas_cap
            .set_target(public_inputs, v.constants_sigmas_cap);
        self.circuit_digest
            .set_target(public_inputs, v.circuit_digest);
    }

    pub fn set_field<C, const D: usize>(
        &self,
        public_inputs: &mut [C::F],
        v: VerifierOnlyCircuitData<C, D>,
    ) where
        C: GenericConfig<D>,
        C::Hasher: Hasher<C::F, Hash = HashOut<C::F>>, {
        self.constants_sigmas_cap
            .set_field(public_inputs, v.constants_sigmas_cap);
        self.circuit_digest
            .set_field(public_inputs, v.circuit_digest);
    }
}

impl Indices for VerifierCircuitTargetIndex {
    type SourceTarget = VerifierCircuitTarget;
    type Target = VerifierCircuitTarget;

    fn new(public_inputs: &[Target], target: &Self::SourceTarget) -> Self {
        Self::new(public_inputs, target)
    }

    fn get_target(&self, public_inputs: &[Target]) -> Self::Target {
        self.get_target(public_inputs)
    }

    fn set_target(&self, public_inputs: &mut [Target], v: Self::Target) {
        self.set_target(public_inputs, v)
    }
}

impl<C, const D: usize> FieldIndices<C::F> for ConfigMarked<C, D, VerifierCircuitTargetIndex>
where
    C: GenericConfig<D>,
    C::Hasher: Hasher<C::F, Hash = HashOut<C::F>>,
{
    type Field = VerifierOnlyCircuitData<C, D>;

    fn get_field(&self, public_inputs: &[C::F]) -> Self::Field { self.t.get_field(public_inputs) }

    fn set_field(&self, public_inputs: &mut [C::F], v: Self::Field) {
        self.t.set_field(public_inputs, v)
    }
}
