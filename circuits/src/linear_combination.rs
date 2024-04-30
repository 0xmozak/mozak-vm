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
use starky::lookup as starky_lookup;

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

impl<T> ColumnSparse<T> {
    pub fn map<F, U>(self, mut f: F) -> ColumnSparse<U>
    where
        F: FnMut(T) -> U, {
        ColumnSparse {
            lv_linear_combination: self
                .lv_linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, f(c)))
                .collect(),
            nv_linear_combination: self
                .nv_linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, f(c)))
                .collect(),
            constant: f(self.constant),
        }
    }
}

pub fn zip_with<T>(
    left: ColumnSparse<T>,
    right: ColumnSparse<T>,
    mut f: impl FnMut(T, T) -> T,
) -> ColumnSparse<T> {
    let mut zip = |mut slc: Vec<(usize, T)>, mut rlc: Vec<(usize, T)>| {
        slc.sort_by_key(|&(col_idx, _)| col_idx);
        rlc.sort_by_key(|&(col_idx, _)| col_idx);
        slc.into_iter()
            .merge_join_by(rlc, |(l, _), (r, _)| l.cmp(r))
            .map(|item| {
                item.reduce(|(idx0, c0), (idx1, c1)| {
                    assert_eq!(idx0, idx1);
                    (idx0, f(c0, c1))
                })
            })
            .collect()
    };

    ColumnSparse {
        lv_linear_combination: zip(left.lv_linear_combination, right.lv_linear_combination),
        nv_linear_combination: zip(left.nv_linear_combination, right.nv_linear_combination),
        constant: f(left.constant, right.constant),
    }
}

pub type ColumnI64 = ColumnSparse<i64>;
pub use ColumnI64 as Column;

impl Column {
    #[must_use]
    pub fn to_starky<F: Field>(&self) -> starky_lookup::Column<F> {
        starky_lookup::Column::new(
            self.lv_linear_combination
                .iter()
                .map(|&(c, f)| (c, F::from_noncanonical_i64(f)))
                .collect(),
            self.nv_linear_combination
                .iter()
                .map(|&(c, f)| (c, F::from_noncanonical_i64(f)))
                .collect(),
            F::from_noncanonical_i64(self.constant),
        )
    }
}

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

impl<F: Neg<Output = F>> Neg for ColumnSparse<F> {
    type Output = Self;

    fn neg(self) -> Self::Output { self.map(Neg::neg) }
}

impl<F: Add<F, Output = F>> Add<Self> for ColumnSparse<F> {
    type Output = Self;

    fn add(self, other: Self) -> Self { zip_with(self, other, Add::add) }
}

impl<F: Add<F, Output = F> + Copy> Add<Self> for &ColumnSparse<F> {
    type Output = ColumnSparse<F>;

    fn add(self, other: Self) -> Self::Output { self.clone() + other.clone() }
}

impl<F: Add<F, Output = F> + Copy> Add<ColumnSparse<F>> for &ColumnSparse<F> {
    type Output = ColumnSparse<F>;

    fn add(self, other: ColumnSparse<F>) -> Self::Output { self.clone() + other }
}

impl<F: Add<F, Output = F> + Copy> Add<&Self> for ColumnSparse<F> {
    type Output = ColumnSparse<F>;

    fn add(self, other: &Self) -> Self::Output { self + other.clone() }
}

impl<F: Add<F, Output = F>> Add<F> for ColumnSparse<F> {
    type Output = Self;

    fn add(self, constant: F) -> Self {
        Self {
            constant: self.constant + constant,
            ..self
        }
    }
}

impl<F: Add<F, Output = F> + Copy> Add<F> for &ColumnSparse<F> {
    type Output = ColumnSparse<F>;

    fn add(self, constant: F) -> ColumnSparse<F> { self.clone() + constant }
}

impl<F: Add<F, Output = F> + Neg<Output = F> + Copy> Sub<Self> for ColumnSparse<F> {
    type Output = Self;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn sub(self, other: Self) -> Self::Output { self + other.neg() }
}

impl<F: Copy + Mul<F, Output = F>> Mul<F> for ColumnSparse<F> {
    type Output = Self;

    fn mul(self, factor: F) -> Self { self.map(|c| c * factor) }
}

impl<F: Copy + Mul<F, Output = F>> Mul<F> for &ColumnSparse<F> {
    type Output = ColumnSparse<F>;

    fn mul(self, factor: F) -> ColumnSparse<F> { self.clone() * factor }
}

impl<F: Add<F, Output = F> + Default> Sum<ColumnSparse<F>> for ColumnSparse<F> {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|x, y| x + y).unwrap_or_default()
    }
}

impl Column {
    // TODO(Matthias): add a `to_field` to `Table` as well.
    pub fn to_field<F: Field>(&self) -> ColumnSparse<F> {
        self.clone().map(F::from_noncanonical_i64)
    }
}

impl<F: Field> ColumnSparse<F> {
    /// Evaluate on a row of a table given in column-major form.
    #[must_use]
    pub fn eval_table(&self, table: &[PolynomialValues<F>], row: usize) -> F {
        self.lv_linear_combination
            .iter()
            .map(|&(c, f)| table[c].values[row] * f)
            .sum::<F>()
            + self
                .nv_linear_combination
                .iter()
                .map(|&(c, f)| table[c].values[(row + 1) % table[c].values.len()] * f)
                .sum::<F>()
            + self.constant
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
