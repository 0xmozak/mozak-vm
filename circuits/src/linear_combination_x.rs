// use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};

// use std::borrow::Borrow;
// use std::ops::Index;

// use itertools::{chain, izip, Itertools};
// use plonky2::field::extension::{Extendable, FieldExtension};
// use plonky2::field::packed::PackedField;
// use plonky2::field::polynomial::PolynomialValues;
// use plonky2::field::types::Field;
// use plonky2::hash::hash_types::RichField;
// use plonky2::iop::ext_target::ExtensionTarget;
// use plonky2::plonk::circuit_builder::CircuitBuilder;

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
