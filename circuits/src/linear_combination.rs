use std::borrow::Borrow;
use std::ops::Index;

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
    lv_linear_combination: Vec<(usize, F)>,
    nv_linear_combination: Vec<(usize, F)>,
    constant: F,
}

impl<F: Field> Column<F> {
    #[must_use]
    pub fn always() -> Self {
        Column {
            lv_linear_combination: vec![],
            nv_linear_combination: vec![],
            constant: F::ONE,
        }
    }

    #[must_use]
    pub fn not(c: usize) -> Self {
        Self {
            lv_linear_combination: vec![],
            nv_linear_combination: vec![(c, F::NEG_ONE)],
            constant: F::ONE,
        }
    }

    #[must_use]
    pub fn single(c: usize) -> Self {
        Self {
            lv_linear_combination: vec![],
            nv_linear_combination: vec![(c, F::ONE)],
            constant: F::ZERO,
        }
    }

    #[must_use]
    pub fn single_prev(c: usize) -> Self {
        Self {
            lv_linear_combination: vec![(c, F::ONE)],
            nv_linear_combination: vec![],
            constant: F::ZERO,
        }
    }

    #[must_use]
    pub fn single_diff(c: usize) -> Self {
        Self {
            lv_linear_combination: vec![(c, -F::ONE)],
            nv_linear_combination: vec![(c, F::ONE)],
            constant: F::ZERO,
        }
    }

    pub fn singles<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Vec<Self> {
        cs.into_iter().map(|c| Self::single(*c.borrow())).collect()
    }

    pub fn singles_diff<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Vec<Self> {
        cs.into_iter().map(|c| Self::single_diff(*c.borrow())).collect()
    }

    #[must_use]
    pub fn many<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            lv_linear_combination: vec![],
            nv_linear_combination: cs.into_iter().map(|c| (*c.borrow(), F::ONE)).collect(),
            constant: F::ZERO,
        }
    }

    #[must_use]
    pub fn ascending_sum<I: IntoIterator<Item = impl Borrow<usize>>>(cs: I) -> Self {
        Column {
            lv_linear_combination: vec![],
            nv_linear_combination: cs
                .into_iter()
                .enumerate()
                .map(|(i, c)| (*c.borrow(), F::from_canonical_usize(i)))
                .collect(),
            constant: F::ZERO,
        }
    }

    pub fn eval<FE, P, const D: usize, V>(&self, lv: &V, nv: &V) -> P
    where
        FE: FieldExtension<D, BaseField = F>,
        P: PackedField<Scalar = FE>,
        V: Index<usize, Output = P> + ?Sized, {
        self.lv_linear_combination
            .iter()
            .map(|&(c, f)| lv[c] * FE::from_basefield(f))
            .sum::<P>()
            + self
                .nv_linear_combination
                .iter()
                .map(|&(c, f)| nv[c] * FE::from_basefield(f))
                .sum::<P>()
            + FE::from_basefield(self.constant)
    }

    /// Evaluate on an row of a table given in column-major form.
    pub fn eval_table(&self, table: &[PolynomialValues<F>], row: usize) -> F {
        self.lv_linear_combination
            .iter()
            .map(|&(c, f)| table[c].values[(row + table[c].values.len() - 1) % table[c].values.len()] * f)
            .sum::<F>()
            + self
                .nv_linear_combination
                .iter()
                .map(|&(c, f)| table[c].values[row] * f)
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
            .nv_linear_combination
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
