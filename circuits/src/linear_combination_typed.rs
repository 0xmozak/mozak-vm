use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};

use itertools::izip;

use crate::columns_view::Zip;

/// Represent a linear combination of columns.
///
/// `InputColumns` could be eg `InputOutputMemory<i64>` or other stark.  We use
/// a 'dense' representation.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct ColumnWithTypedInput<InputColumns> {
    /// Linear combination of the local row
    pub lv_linear_combination: InputColumns,
    /// Linear combination of the next row
    pub nv_linear_combination: InputColumns,
    /// Constant of linear combination
    pub constant: i64,
}

impl<InputColumns> ColumnWithTypedInput<InputColumns> {
    /// Flip lv and nv
    #[must_use]
    pub fn flip(self) -> Self {
        ColumnWithTypedInput {
            lv_linear_combination: self.nv_linear_combination,
            nv_linear_combination: self.lv_linear_combination,
            constant: self.constant,
        }
    }
}

impl<C> Neg for ColumnWithTypedInput<C>
where
    C: Neg<Output = C>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            lv_linear_combination: -self.lv_linear_combination,
            nv_linear_combination: -self.nv_linear_combination,
            constant: self.constant.checked_neg().expect("negation overflow"),
        }
    }
}

impl<C> Add<Self> for ColumnWithTypedInput<C>
where
    C: Add<Output = C>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            lv_linear_combination: self.lv_linear_combination + other.lv_linear_combination,
            nv_linear_combination: self.nv_linear_combination + other.nv_linear_combination,
            constant: self
                .constant
                .checked_add(other.constant)
                .expect("addition overflow"),
        }
    }
}

impl<C> Add<i64> for ColumnWithTypedInput<C>
where
    C: Add<Output = C>,
{
    type Output = Self;

    fn add(self, other: i64) -> Self {
        Self {
            lv_linear_combination: self.lv_linear_combination,
            nv_linear_combination: self.nv_linear_combination,
            constant: self.constant.checked_add(other).expect("addition overflow"),
        }
    }
}

impl<C> Sub<Self> for ColumnWithTypedInput<C>
where
    C: Sub<Output = C>,
{
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            lv_linear_combination: self.lv_linear_combination - other.lv_linear_combination,
            nv_linear_combination: self.nv_linear_combination - other.nv_linear_combination,
            constant: self
                .constant
                .checked_sub(other.constant)
                .expect("subtraction overflow"),
        }
    }
}

impl<C> Mul<i64> for ColumnWithTypedInput<C>
where
    C: Mul<i64, Output = C>,
{
    type Output = Self;

    fn mul(self, other: i64) -> Self {
        Self {
            lv_linear_combination: self.lv_linear_combination * other,
            nv_linear_combination: self.nv_linear_combination * other,
            constant: self
                .constant
                .checked_mul(other)
                .expect("multiplication overflow"),
        }
    }
}

impl<C> Sum<ColumnWithTypedInput<C>> for ColumnWithTypedInput<C>
where
    Self: Add<Output = Self> + Default,
{
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { iter.fold(Self::default(), Add::add) }
}

impl<'a, C: Copy> Sum<&'a Self> for ColumnWithTypedInput<C>
where
    Self: Add<Output = Self> + Default,
{
    #[inline]
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().fold(Self::default(), Add::add)
    }
}

impl<C> ColumnWithTypedInput<C>
where
    ColumnWithTypedInput<C>: Default,
{
    #[must_use]
    pub fn constant(constant: i64) -> Self {
        ColumnWithTypedInput {
            constant,
            ..Default::default()
        }
    }
}
impl<C: Default> From<C> for ColumnWithTypedInput<C> {
    fn from(lv_linear_combination: C) -> Self { Self::now(lv_linear_combination) }
}

impl<C: Default> ColumnWithTypedInput<C> {
    pub fn now(lv_linear_combination: C) -> Self {
        Self {
            lv_linear_combination,
            nv_linear_combination: C::default(),
            constant: Default::default(),
        }
    }

    pub fn next(nv_linear_combination: C) -> Self {
        Self {
            nv_linear_combination,
            lv_linear_combination: C::default(),
            constant: Default::default(),
        }
    }
}

impl<C: Default + Zip<i64>> ColumnWithTypedInput<C>
where
    Self: Default
        + Sub<Output = Self>
        + Mul<i64, Output = Self>
        + Add<Output = Self>
        + Neg<Output = Self>
        + Sum,
    C: IntoIterator<Item = i64>,
{
    #[must_use]
    pub fn reduce_with_powers<I>(terms: I, alpha: i64) -> Self
    where
        I: IntoIterator<Item = Self>,
        I::IntoIter: DoubleEndedIterator, {
        terms
            .into_iter()
            .rev()
            .fold(Self::default(), |acc, term| acc * alpha + term)
    }

    #[must_use]
    pub fn ascending_sum<I>(cs: I) -> Self
    where
        I: IntoIterator<Item = Self>, {
        izip!(cs, 0..).map(|(c, i)| c * i).sum()
    }
}
