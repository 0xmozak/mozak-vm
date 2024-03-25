use std::ops::Index;

use itertools::{chain, Itertools};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use crate::cross_table_lookup::ColumnTyped;

/// Represent a linear combination of columns.
#[derive(Clone, Debug, Default)]
pub struct Column {
    /// Linear combination of the local row
    pub lv_linear_combination: Vec<(usize, i64)>,
    /// Linear combination of the next row
    pub nv_linear_combination: Vec<(usize, i64)>,
    /// Constant of linear combination
    pub constant: i64,
}

impl<I: IntoIterator<Item = i64>> From<ColumnTyped<I>> for Column {
    fn from(colx: ColumnTyped<I>) -> Self {
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
