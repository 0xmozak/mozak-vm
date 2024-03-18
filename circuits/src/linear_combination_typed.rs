use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};

use itertools::izip;
use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;

use crate::columns_view::Zip;
// TODO(Matthias): consider making a ColMap for ColumnTyped as well.
// use crate::linear_combination::Column;

// TODO(Matthias): figure out why this one isn't used!

// pub fn to_untyped<X: IntoIterator<Item = i64>>(input: ColumnTyped<X>) ->
// Column {     // TODO(Matthias): we could filter out zero coefficients here,
// if we wanted to.     Column {
//         lv_linear_combination: input
//             .lv_linear_combination
//             .into_iter()
//             .enumerate()
//             .collect(),
//         nv_linear_combination: input
//             .nv_linear_combination
//             .into_iter()
//             .enumerate()
//             .collect(),
//         constant: input.constant,
//     }
// }

/// Represent a linear combination of columns.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct ColumnTyped<C> {
    /// Linear combination of the local row
    pub lv_linear_combination: C,
    /// Linear combination of the next row
    pub nv_linear_combination: C,
    /// Constant of linear combination
    pub constant: i64,
}

/// Flip lv and nv
pub fn flip<C>(col: ColumnTyped<C>) -> ColumnTyped<C> {
    ColumnTyped {
        lv_linear_combination: col.nv_linear_combination,
        nv_linear_combination: col.lv_linear_combination,
        constant: col.constant,
    }
}

impl<C> Neg for ColumnTyped<C>
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

impl<C> Add<Self> for ColumnTyped<C>
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

impl<C> Add<i64> for ColumnTyped<C>
where
    C: Add<Output = C>,
{
    type Output = Self;

    #[allow(clippy::similar_names)]
    fn add(self, other: i64) -> Self {
        Self {
            lv_linear_combination: self.lv_linear_combination,
            nv_linear_combination: self.nv_linear_combination,
            constant: self.constant.checked_add(other).expect("addition overflow"),
        }
    }
}

impl<C> Sub<Self> for ColumnTyped<C>
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

impl<C> Mul<i64> for ColumnTyped<C>
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

impl<C> Sum<ColumnTyped<C>> for ColumnTyped<C>
where
    Self: Add<Output = Self> + Default,
{
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { iter.fold(Self::default(), Add::add) }
}

impl<C> ColumnTyped<C>
where
    ColumnTyped<C>: Default,
{
    #[must_use]
    pub fn always() -> Self {
        ColumnTyped {
            constant: 1,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn constant(constant: i64) -> Self {
        ColumnTyped {
            constant,
            ..Default::default()
        }
    }
}
impl<C: Default> From<C> for ColumnTyped<C> {
    fn from(lv_linear_combination: C) -> Self { Self::now(lv_linear_combination) }
}

// impl<C: Default, I> From<I> for ColumnTyped<C> where I: :
// IntoIterator<Item=C>{     fn from(lvs: C) -> Self {
//         lvs.into_iter().map(Self::from).sum()
//     }
// }

impl<C: Default> ColumnTyped<C> {
    pub const fn now(lv_linear_combination: C) -> Self {
        Self {
            lv_linear_combination,
            nv_linear_combination: C::default(),
            constant: Default::default(),
        }
    }

    pub const fn next(nv_linear_combination: C) -> Self {
        Self {
            nv_linear_combination,
            lv_linear_combination: C::default(),
            constant: Default::default(),
        }
    }
}

impl<C: Default + Zip<i64>> ColumnTyped<C>
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

    // TODO(Matthias): make this one more typed: V and C should agree, sort of.
    pub fn eval<F: Field, FE, P, const D: usize, V>(self, lv: V, nv: V) -> P
    where
        FE: FieldExtension<D, BaseField = F>,
        P: PackedField<Scalar = FE>,
        V: IntoIterator<Item = P>, {
        izip!(lv, self.lv_linear_combination)
            .map(|(lv1, f)| lv1 * FE::from_noncanonical_i64(f))
            .sum::<P>()
            + izip!(nv, self.nv_linear_combination)
                .map(|(nv1, f)| nv1 * FE::from_noncanonical_i64(f))
                .sum::<P>()
            + FE::from_noncanonical_i64(self.constant)
    }

    /// Evaluate on a row of a table given in column-major form.
    #[allow(clippy::cast_possible_wrap)]
    #[must_use]
    pub fn eval_table<F: Field>(self, table: &[PolynomialValues<F>], row: usize) -> F {
        izip!(table, self.lv_linear_combination)
            .map(|(t, f)| t.values[row] * F::from_noncanonical_i64(f))
            .sum::<F>()
            + izip!(table, self.nv_linear_combination)
                .map(|(t, f)| t.values[(row + 1) % t.values.len()] * F::from_noncanonical_i64(f))
                .sum::<F>()
            + F::from_noncanonical_i64(self.constant)
    }

    /// Evaluate on an row of a table
    #[allow(clippy::similar_names)]
    pub fn eval_row<I, F: Field>(self, lv_row: I, nv_row: I) -> F
    where
        I: IntoIterator<Item = F>, {
        izip!(lv_row, self.lv_linear_combination)
            .map(|(lv1, f)| lv1 * F::from_noncanonical_i64(f))
            .sum::<F>()
            + izip!(nv_row, self.nv_linear_combination)
                .map(|(nv1, f)| nv1 * F::from_noncanonical_i64(f))
                .sum::<F>()
            + F::from_noncanonical_i64(self.constant)
    }
}
