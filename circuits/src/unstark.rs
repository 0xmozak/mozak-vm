use core::fmt::Debug;
use std::fmt::Display;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::GenerateConstraints;

impl<'a, F, NAME, T: 'a, const D: usize, Columns, const COLUMNS: usize> GenerateConstraints<'a, T>
    for Unstark<F, NAME, { D }, Columns, { COLUMNS }>
{
    type PublicInputs<E: 'a> = NoColumns<E>;
    type View<E: 'a> = ShadowColumns<E, { COLUMNS }>;
}

/// Template for a STARK with zero internal constraints. Use this if the STARK
/// itself does not need any built-in constraints, but rely on cross table
/// lookups for provability.
#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct Unstark<F, NAME, const D: usize, Columns, const COLUMNS: usize> {
    pub _f: PhantomData<F>,
    pub _name: PhantomData<NAME>,
    pub _d: PhantomData<Columns>,
}

impl<F, NAME: Default + Debug, const D: usize, Columns, const COLUMNS: usize> Display
    for Unstark<F, NAME, D, Columns, COLUMNS>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", NAME::default())
    }
}

impl<F, NAME, const D: usize, Columns, const COLUMNS: usize> HasNamedColumns
    for Unstark<F, NAME, D, Columns, COLUMNS>
{
    type Columns = Columns;
}

const PUBLIC_INPUTS: usize = 0;

impl<
        F: RichField + Extendable<D>,
        NAME: Sync,
        const D: usize,
        Columns: Sync + NumberOfColumns,
        const COLUMNS: usize,
    > Stark<F, D> for Unstark<F, NAME, D, Columns, COLUMNS>
{
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        _vars: &Self::EvaluationFrame<FE, P, D2>,
        _constraint_consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _constraint_consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
    }

    fn constraint_degree(&self) -> usize { 3 }
}

// Simple marco to create a type holding the name for the Unstark
macro_rules! impl_name {
    ($alias:ident, $name:ident) => {
        mod name {
            #[derive(Default, Debug, Clone, Copy)]
            pub struct $name {}
        }

        use name::$name as $alias;
    };
}

pub(crate) use impl_name;

pub type NoColumns<T> = ShadowColumns<T, 0>;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ShadowColumns<T, const N: usize> {
    _columns: [T; N],
}

// Manually implement columns_view_impl! for Columns
impl<T, const N: usize> crate::columns_view::ColumnViewImplHider<ShadowColumns<T, N>> {
    const fn from_array(value: [T; N]) -> ShadowColumns<T, N> {
        unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
    }

    const fn into_array(v: ShadowColumns<T, N>) -> [T; N] {
        unsafe { crate::columns_view::transmute_without_compile_time_size_checks(v) }
    }

    const fn from_array_ref(value: &[T; N]) -> &ShadowColumns<T, N> {
        unsafe { crate::columns_view::transmute_ref(value) }
    }

    const fn array_ref(v: &ShadowColumns<T, N>) -> &[T; N] {
        unsafe { crate::columns_view::transmute_ref(v) }
    }
}

impl<T, const N: usize> ShadowColumns<T, N> {
    pub const fn from_array(value: [T; N]) -> Self {
        crate::columns_view::ColumnViewImplHider::<Self>::from_array(value)
    }

    #[must_use]
    pub const fn into_array(self) -> [T; N] {
        crate::columns_view::ColumnViewImplHider::<Self>::into_array(self)
    }

    pub const fn from_array_ref(value: &[T; N]) -> &Self {
        crate::columns_view::ColumnViewImplHider::<Self>::from_array_ref(value)
    }

    #[must_use]
    pub const fn array_ref(&self) -> &[T; N] {
        crate::columns_view::ColumnViewImplHider::<Self>::array_ref(self)
    }

    pub fn iter(&self) -> std::slice::Iter<T> { self.array_ref().iter() }

    // At the moment we only use `map` Instruction,
    // so it's dead code for the other callers of `columns_view_impl`.
    // TODO(Matthias): remove this marker, once we use it for the other structs,
    // too.
    #[allow(dead_code)]
    pub fn map<B, F>(self, f: F) -> ShadowColumns<B, N>
    where
        F: FnMut(T) -> B, {
        ShadowColumns::from_array(self.into_array().map(f))
    }
}

impl<Item, const N: usize> crate::columns_view::Zip<Item> for ShadowColumns<Item, N> {
    fn zip_with<F>(self, other: Self, mut f: F) -> Self
    where
        F: FnMut(Item, Item) -> Item, {
        ShadowColumns::from_array({
            let mut a = self.into_iter();
            let mut b = other.into_iter();
            core::array::from_fn(move |_| f(a.next().unwrap(), b.next().unwrap()))
        })
    }
}

impl<T, const N: usize> crate::columns_view::NumberOfColumns for ShadowColumns<T, N> {
    // `u8` is guaranteed to have a `size_of` of 1.
    const NUMBER_OF_COLUMNS: usize = N;
}

impl<T, const N: usize> From<[T; N]> for ShadowColumns<T, N> {
    fn from(value: [T; N]) -> Self { Self::from_array(value) }
}

impl<T, const N: usize> From<ShadowColumns<T, N>> for [T; N] {
    fn from(value: ShadowColumns<T, N>) -> Self { value.into_array() }
}

impl<'a, T, const N: usize> From<&'a [T]> for &'a ShadowColumns<T, N> {
    fn from(value: &'a [T]) -> Self {
        let value: &[T; N] = value.try_into().expect("slice of correct length");
        ShadowColumns::from_array_ref(value)
    }
}

impl<T, const N: usize> std::borrow::Borrow<[T]> for ShadowColumns<T, N> {
    fn borrow(&self) -> &[T] { self.array_ref() }
}

impl<T, I, const N: usize> std::ops::Index<I> for ShadowColumns<T, N>
where
    [T]: std::ops::Index<I>,
{
    type Output = <[T] as std::ops::Index<I>>::Output;

    fn index(&self, index: I) -> &Self::Output { &self.array_ref()[index] }
}

impl<T, const N: usize> std::iter::IntoIterator for ShadowColumns<T, N> {
    type IntoIter = std::array::IntoIter<T, { N }>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter { self.into_array().into_iter() }
}

impl<'a, T, const N: usize> std::iter::IntoIterator for &'a ShadowColumns<T, N> {
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter { self.iter() }
}

impl<T: std::fmt::Debug, const N: usize> std::iter::FromIterator<T> for ShadowColumns<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let vec: arrayvec::ArrayVec<T, N> = iter.into_iter().collect();
        let array = vec.into_inner().expect("iterator of correct length");
        Self::from_array(array)
    }
}

impl<const N: usize> core::ops::Neg for ShadowColumns<i64, N> {
    type Output = Self;

    fn neg(self) -> Self::Output { self.map(|x| x.checked_neg().expect("negation overflow")) }
}

impl<const N: usize> core::ops::Add<ShadowColumns<i64, N>> for ShadowColumns<i64, N> {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        crate::columns_view::Zip::zip_with(self, other, |a, b| {
            a.checked_add(b).expect("addition overflow")
        })
    }
}

impl<const N: usize> core::ops::Sub<ShadowColumns<i64, N>> for ShadowColumns<i64, N> {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        crate::columns_view::Zip::zip_with(self, other, |a, b| {
            a.checked_sub(b).expect("subtraction overflow")
        })
    }
}

impl<const N: usize> core::ops::Mul<i64> for ShadowColumns<i64, N> {
    type Output = Self;

    fn mul(self, other: i64) -> Self::Output {
        self.map(|x| x.checked_mul(other).expect("multiplication overflow"))
    }
}

impl<const N: usize> core::iter::Sum<ShadowColumns<i64, N>> for ShadowColumns<i64, N> {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), core::ops::Add::add)
    }
}

// Some of our tables have columns that are specifiecd as arrays that are bigger
// than 32 elements.  Thus default derivation doesn't work, so we do it manually
// here.
impl<F: Default, const N: usize> Default for ShadowColumns<F, N> {
    fn default() -> Self { ShadowColumns::from_array(core::array::from_fn(|_| Default::default())) }
}
