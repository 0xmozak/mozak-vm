use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use std::borrow::Borrow;
use std::ops::Index;

use itertools::{EitherOrBoth, Itertools};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;

/// Represent a linear combination of columns.
#[derive(Clone, Debug, Default)]
pub struct Column<F: Field> {
    linear_combination: Vec<(usize, F)>,
    constant: F,
}

impl<F: Field> Neg for Column<F> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            linear_combination: self
                .linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, -c))
                .collect(),
            constant: -self.constant,
        }
    }
}

impl<F: Field> Add<Self> for Column<F> {
    type Output = Self;

    #[allow(clippy::similar_names)]
    fn add(
        self,
        Column {
            linear_combination: mut rlc,
            constant: rc,
        }: Self,
    ) -> Self {
        let Self {
            linear_combination: mut slc,
            constant: sc,
        } = self;
        slc.sort_by_key(|&(col_idx, _)| col_idx);
        rlc.sort_by_key(|&(col_idx, _)| col_idx);

        let linear_combination = slc
            .into_iter()
            .merge_join_by(rlc, |(l, _), (r, _)| l.cmp(r))
            .map(|x| match x {
                EitherOrBoth::Left(pair) | EitherOrBoth::Right(pair) => pair,
                EitherOrBoth::Both((idx0, c0), (idx1, c1)) => {
                    assert_eq!(idx0, idx1);
                    (idx0, c0 + c1)
                }
            })
            .collect();

        Self {
            linear_combination,
            constant: sc + rc,
        }
    }
}

impl<F: Field> Add<Self> for &Column<F> {
    type Output = Column<F>;

    fn add(self, other: Self) -> Self::Output { self.clone() + other.clone() }
}

impl<F: Field> Add<Column<F>> for &Column<F> {
    type Output = Column<F>;

    fn add(self, other: Column<F>) -> Self::Output { self.clone() + other }
}

impl<F: Field> Add<&Self> for Column<F> {
    type Output = Column<F>;

    fn add(self, other: &Self) -> Self::Output { self + other.clone() }
}

impl<F: Field> Add<F> for Column<F> {
    type Output = Self;

    fn add(self, constant: F) -> Self {
        Self {
            linear_combination: self.linear_combination,
            constant: self.constant + constant,
        }
    }
}

impl<F: Field> Add<F> for &Column<F> {
    type Output = Column<F>;

    fn add(self, constant: F) -> Column<F> { self.clone() + constant }
}

impl<F: Field> Sub<Self> for Column<F> {
    type Output = Self;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn sub(self, other: Self) -> Self::Output { self.clone() + other.neg() }
}

impl<F: Field> Mul<F> for Column<F> {
    type Output = Self;

    fn mul(self, factor: F) -> Self {
        Self {
            linear_combination: self
                .linear_combination
                .into_iter()
                .map(|(idx, c)| (idx, factor * c))
                .collect(),
            constant: factor * self.constant,
        }
    }
}

impl<F: Field> Mul<F> for &Column<F> {
    type Output = Column<F>;

    fn mul(self, factor: F) -> Column<F> { self.clone() * factor }
}

impl<F: Field> Sum<Column<F>> for Column<F> {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|x, y| x + y).unwrap_or_default()
    }
}

impl<F: Field> Sum<usize> for Column<F> {
    #[inline]
    fn sum<I: Iterator<Item = usize>>(iter: I) -> Self { iter.map(Self::from).sum() }
}

// TODO: implement other traits like Sub, MulAssign, etc as we need them.

impl<F: Field> From<usize> for Column<F> {
    fn from(idx: usize) -> Self {
        Self {
            linear_combination: vec![(idx, F::ONE)],
            constant: F::ZERO,
        }
    }
}

impl<F: Field> Column<F> {
    #[must_use]
    pub fn always() -> Self {
        Column {
            linear_combination: vec![],
            constant: F::ONE,
        }
    }

    #[must_use]
    pub fn not(c: usize) -> Self {
        Self {
            linear_combination: vec![(c, F::NEG_ONE)],
            constant: F::ONE,
        }
    }

    #[must_use]
    pub fn single(idx: usize) -> Self { idx.into() }

    pub fn singles<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Vec<Self> {
        cs.into_iter().map(|c| Self::single(*c.borrow())).collect()
    }

    #[must_use]
    pub fn many<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            linear_combination: cs.into_iter().map(|c| (*c.borrow(), F::ONE)).collect(),
            constant: F::ZERO,
        }
    }

    #[must_use]
    pub fn shift_combination<I: IntoIterator<Item = impl Borrow<usize>>>(
        cs: I,
        shift_amount: usize,
    ) -> Self {
        Column {
            linear_combination: cs
                .into_iter()
                .enumerate()
                .map(|(i, c)| (*c.borrow(), F::from_canonical_usize(1 << shift_amount * i)))
                .collect(),
            constant: F::ZERO,
        }
    }

    #[must_use]
    pub fn ascending_sum<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            linear_combination: cs
                .into_iter()
                .enumerate()
                .map(|(i, c)| (*c.borrow(), F::from_canonical_usize(i)))
                .collect(),
            constant: F::ZERO,
        }
    }

    pub fn eval<FE, P, const D: usize, V>(&self, v: &V) -> P
    where
        FE: FieldExtension<D, BaseField = F>,
        P: PackedField<Scalar = FE>,
        V: Index<usize, Output = P> + ?Sized, {
        self.linear_combination
            .iter()
            .map(|&(c, f)| v[c] * FE::from_basefield(f))
            .sum::<P>()
            + FE::from_basefield(self.constant)
    }

    /// Evaluate on an row of a table given in column-major form.
    pub fn eval_table(&self, table: &[PolynomialValues<F>], row: usize) -> F {
        self.linear_combination
            .iter()
            .map(|&(c, f)| table[c].values[row] * f)
            .sum::<F>()
            + self.constant
    }

    /// Evaluate on an row of a table
    pub fn eval_row(&self, row: &impl Index<usize, Output = F>) -> F {
        self.linear_combination
            .iter()
            .map(|&(c, f)| row[c] * f)
            .sum::<F>()
            + self.constant
    }

    pub fn eval_circuit<const D: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        v: &[ExtensionTarget<D>],
    ) -> ExtensionTarget<D>
    where
        F: RichField + Extendable<D>, {
        let pairs = self
            .linear_combination
            .iter()
            .map(|&(c, f)| {
                (
                    v[c],
                    builder.constant_extension(F::Extension::from_basefield(f)),
                )
            })
            .collect::<Vec<_>>();
        let constant = builder.constant_extension(F::Extension::from_basefield(self.constant));
        builder.inner_product_extension(F::ONE, constant, pairs)
    }
}
