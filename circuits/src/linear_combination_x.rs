use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};

use itertools::izip;

use crate::columns_view::Zip;

/// Represent a linear combination of columns.
#[derive(Clone, Debug, Default)]
pub struct ColumnX<C> {
    /// Linear combination of the local row
    lv_linear_combination: C,
    /// Linear combination of the next row
    nv_linear_combination: C,
    /// Constant of linear combination
    constant: i64,
}

/// Flip lv and nv
pub fn flip<C>(col: ColumnX<C>) -> ColumnX<C> {
    ColumnX {
        lv_linear_combination: col.nv_linear_combination,
        nv_linear_combination: col.lv_linear_combination,
        constant: col.constant,
    }
}

impl<C> Neg for ColumnX<C>
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

impl<C> Add<Self> for ColumnX<C>
where
    C: Add<Output = C>,
{
    type Output = Self;

    #[allow(clippy::similar_names)]
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

impl<C> Sub<Self> for ColumnX<C>
where
    C: Sub<Output = C>,
{
    type Output = Self;

    #[allow(clippy::similar_names)]
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

impl<C> Mul<i64> for ColumnX<C>
where
    C: Mul<i64, Output = C>,
{
    type Output = Self;

    #[allow(clippy::similar_names)]
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

impl<C> Sum<ColumnX<C>> for ColumnX<C>
where
    Self: Add<Output = Self> + Default,
{
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { iter.fold(Self::default(), Add::add) }
}

impl<C> ColumnX<C>
where
    ColumnX<C>: Default,
{
    #[must_use]
    pub fn always() -> Self {
        ColumnX {
            constant: 1,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn constant(constant: i64) -> Self {
        ColumnX {
            constant,
            ..Default::default()
        }
    }
}
impl<C> From<C> for ColumnX<C>
where
    ColumnX<C>: Default,
{
    fn from(lv_linear_combination: C) -> Self {
        Self {
            lv_linear_combination,
            ..Default::default()
        }
    }
}

impl<C: Default + Zip<i64>> ColumnX<C>
where
    Self: Default
        + Sub<Output = Self>
        + Mul<i64, Output = Self>
        + Add<Output = Self>
        + Neg<Output = Self>
        + Sum,
{
    pub fn next(nv_linear_combination: C) -> Self {
        Self {
            nv_linear_combination,
            ..Default::default()
        }
    }

    /// This is useful for `not`: `all_lv - Self::from(my_column)`
    // We could also implement this as a `sum` over COL_MAP, but the types are more annoying to get
    // right.
    #[must_use]
    pub fn all_lv() -> Self {
        ColumnX {
            lv_linear_combination: C::default().map1(|_| 1),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn not(c: C) -> Self { Self::all_lv() - Self::from(c) }

    // TODO(Matthias): Be careful about overflow here?
    #[must_use]
    pub fn reduce_with_powers<I>(terms: I, alpha: i64) -> Self
    where
        I: IntoIterator<Item = Self>,
        <I as IntoIterator>::IntoIter: DoubleEndedIterator, {
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
