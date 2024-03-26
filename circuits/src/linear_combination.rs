use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use std::ops::Index;

use itertools::{chain, Itertools};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use crate::cross_table_lookup::ColumnWithTypedInput;

/// Represent a linear combination of columns.
#[derive(Clone, Debug, Default)]
pub struct ColumnSparse<F> {
    /// Linear combination of the local row
    pub lv_linear_combination: Vec<(usize, F)>,
    /// Linear combination of the next row
    pub nv_linear_combination: Vec<(usize, F)>,
    /// Constant of linear combination
    pub constant: F,
}

pub type ColumnI64 = ColumnSparse<i64>;
pub use ColumnI64 as Column;

impl<I: IntoIterator<Item = i64>> From<ColumnWithTypedInput<I>> for Column {
    fn from(colx: ColumnWithTypedInput<I>) -> Self {
        fn to_sparse(v: impl IntoIterator<Item = i64>) -> Vec<(usize, i64)> {
            v.into_iter()
                .enumerate()
                .filter(|(_i, coefficient)| coefficient != &0)
                .collect()
        }
        Self {
            lv_linear_combination: to_sparse(colx.lv_linear_combination),
            nv_linear_combination: to_sparse(colx.nv_linear_combination),
            constant: colx.constant,
        }
    }
}

impl Neg for Column {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            lv_linear_combination: self
                .lv_linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, c.checked_neg().unwrap()))
                .collect(),
            nv_linear_combination: self
                .nv_linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, c.checked_neg().unwrap()))
                .collect(),
            constant: -self.constant,
        }
    }
}

impl Add<Self> for Column {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let add_lc = |mut slc: Vec<(usize, i64)>, mut rlc: Vec<(usize, i64)>| {
            slc.sort_by_key(|&(col_idx, _)| col_idx);
            rlc.sort_by_key(|&(col_idx, _)| col_idx);
            slc.into_iter()
                .merge_join_by(rlc, |(l, _), (r, _)| l.cmp(r))
                .map(|item| {
                    item.reduce(|(idx0, c0), (idx1, c1)| {
                        assert_eq!(idx0, idx1);
                        (idx0, c0.checked_add(c1).unwrap())
                    })
                })
                .collect()
        };

        Self {
            lv_linear_combination: add_lc(self.lv_linear_combination, other.lv_linear_combination),
            nv_linear_combination: add_lc(self.nv_linear_combination, other.nv_linear_combination),
            constant: self.constant + other.constant,
        }
    }
}

impl Add<Self> for &Column {
    type Output = Column;

    fn add(self, other: Self) -> Self::Output { self.clone() + other.clone() }
}

impl Add<Column> for &Column {
    type Output = Column;

    fn add(self, other: Column) -> Self::Output { self.clone() + other }
}

impl Add<&Self> for Column {
    type Output = Column;

    fn add(self, other: &Self) -> Self::Output { self + other.clone() }
}

impl Add<i64> for Column {
    type Output = Self;

    fn add(self, constant: i64) -> Self {
        Self {
            constant: self.constant.checked_add(constant).unwrap(),
            ..self
        }
    }
}

impl Add<i64> for &Column {
    type Output = Column;

    fn add(self, constant: i64) -> Column { self.clone() + constant }
}

impl Sub<Self> for Column {
    type Output = Self;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn sub(self, other: Self) -> Self::Output { self.clone() + other.neg() }
}

impl Mul<i64> for Column {
    type Output = Self;

    fn mul(self, factor: i64) -> Self {
        Self {
            lv_linear_combination: self
                .lv_linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, factor.checked_mul(c).unwrap()))
                .collect(),
            nv_linear_combination: self
                .nv_linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, factor.checked_mul(c).unwrap()))
                .collect(),
            constant: factor.checked_mul(self.constant).unwrap(),
        }
    }
}

impl Mul<i64> for &Column {
    type Output = Column;

    fn mul(self, factor: i64) -> Column { self.clone() * factor }
}

impl Sum<Column> for Column {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|x, y| x + y).unwrap_or_default()
    }
}

impl Column {
    // TODO(Matthias): move the eval* functions into the 'typed' world.
    pub fn eval<F: Field, FE, P, const D: usize, V>(&self, lv: &V, nv: &V) -> P
    where
        FE: FieldExtension<D, BaseField = F>,
        P: PackedField<Scalar = FE>,
        V: Index<usize, Output = P> + ?Sized, {
        self.lv_linear_combination
            .iter()
            .map(|&(c, f)| lv[c] * FE::from_noncanonical_i64(f))
            .sum::<P>()
            + self
                .nv_linear_combination
                .iter()
                .map(|&(c, f)| nv[c] * FE::from_noncanonical_i64(f))
                .sum::<P>()
            + FE::from_noncanonical_i64(self.constant)
    }

    /// Evaluate on a row of a table given in column-major form.
    #[must_use]
    pub fn eval_table<F: Field>(&self, table: &[PolynomialValues<F>], row: usize) -> F {
        self.lv_linear_combination
            .iter()
            .map(|&(c, f)| table[c].values[row] * F::from_noncanonical_i64(f))
            .sum::<F>()
            + self
                .nv_linear_combination
                .iter()
                .map(|&(c, f)| {
                    table[c].values[(row + 1) % table[c].values.len()] * F::from_noncanonical_i64(f)
                })
                .sum::<F>()
            + F::from_noncanonical_i64(self.constant)
    }

    /// Evaluate on an row of a table
    pub fn eval_row<F: Field>(
        &self,
        lv_row: &impl Index<usize, Output = F>,
        nv_row: &impl Index<usize, Output = F>,
    ) -> F {
        self.lv_linear_combination
            .iter()
            .map(|&(c, f)| lv_row[c] * F::from_noncanonical_i64(f))
            .sum::<F>()
            + self
                .nv_linear_combination
                .iter()
                .map(|&(c, f)| nv_row[c] * F::from_noncanonical_i64(f))
                .sum::<F>()
            + F::from_noncanonical_i64(self.constant)
    }

    pub fn eval_circuit<F, const D: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        v: &[ExtensionTarget<D>],
        next_v: &[ExtensionTarget<D>],
    ) -> ExtensionTarget<D>
    where
        F: RichField + Extendable<D>, {
        let pairs = chain!(
            self.lv_linear_combination
                .iter()
                .map(|&(c, f)| { (v[c], f) }),
            self.nv_linear_combination
                .iter()
                .map(|&(c, f)| { (next_v[c], f) })
        )
        .map(|(v, f)| {
            (
                v,
                builder.constant_extension(F::Extension::from_noncanonical_i64(f)),
            )
        })
        .collect_vec();
        let constant =
            builder.constant_extension(F::Extension::from_noncanonical_i64(self.constant));
        builder.inner_product_extension(F::ONE, constant, pairs)
    }
}
