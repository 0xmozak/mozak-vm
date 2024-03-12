use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use std::borrow::Borrow;
use std::ops::Index;

use itertools::{chain, Itertools};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;

/// Represent a linear combination of columns.
#[derive(Clone, Debug, Default)]
pub struct Column {
    /// Linear combination of the local row
    lv_linear_combination: Vec<(usize, i64)>,
    /// Linear combination of the next row
    nv_linear_combination: Vec<(usize, i64)>,
    /// Constant of linear combination
    constant: i64,
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

    #[allow(clippy::similar_names)]
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

impl Sum<usize> for Column {
    #[inline]
    fn sum<I: Iterator<Item = usize>>(iter: I) -> Self { iter.map(Self::from).sum() }
}

// TODO: implement other traits like Sub, MulAssign, etc as we need them.

impl From<usize> for Column {
    fn from(idx: usize) -> Self {
        Self {
            lv_linear_combination: vec![(idx, 1)],
            ..Self::default()
        }
    }
}

impl Column {
    #[must_use]
    pub fn always() -> Self {
        Column {
            constant: 1,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn constant(constant: i64) -> Self {
        Column {
            constant,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn not(c: usize) -> Self {
        Self {
            lv_linear_combination: vec![(c, -1)],
            constant: 1,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn single(c: usize) -> Self {
        Self {
            lv_linear_combination: vec![(c, 1)],
            ..Default::default()
        }
    }

    /// Returns a column whose ith row refers to nv value of ith row of the
    /// column with given index c.
    #[must_use]
    pub fn single_next(c: usize) -> Self {
        Self {
            nv_linear_combination: vec![(c, 1)],
            ..Default::default()
        }
    }

    #[must_use]
    pub fn single_diff(c: usize) -> Self { Self::single_next(c) - Self::single(c) }

    pub fn singles<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Vec<Self> {
        cs.into_iter().map(|c| Self::single(*c.borrow())).collect()
    }

    pub fn singles_next<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Vec<Self> {
        cs.into_iter()
            .map(|c| Self::single_next(*c.borrow()))
            .collect()
    }

    pub fn singles_diff<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Vec<Self> {
        cs.into_iter()
            .map(|c| Self::single_diff(*c.borrow()))
            .collect()
    }

    #[must_use]
    pub fn many<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            lv_linear_combination: cs.into_iter().map(|c| (*c.borrow(), 1)).collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn many_next<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            nv_linear_combination: cs.into_iter().map(|c| (*c.borrow(), 1)).collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn reduce_with_powers(terms: &[Self], alpha: i64) -> Self {
        terms
            .iter()
            .rev()
            .fold(Self::default(), |acc, term| acc * alpha + term)
    }

    #[must_use]
    pub fn ascending_sum<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            lv_linear_combination: cs.into_iter().map(|c| *c.borrow()).zip(0..).collect(),
            ..Default::default()
        }
    }

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

    /// Evaluate on an row of a table given in column-major form.
    #[allow(clippy::cast_possible_wrap)]
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
    #[allow(clippy::similar_names)]
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

    pub fn eval_circuit<F: Field, const D: usize>(
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
