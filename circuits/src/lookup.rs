//! Implementation of the Logup lookup argument.
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use serde::{Deserialize, Serialize};
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lookup {
    /// f_i(x)
    pub(crate) looking_columns: Vec<usize>,
    /// t_i(x)
    pub(crate) looked_column: usize,
    /// m_i(x)
    pub(crate) multiplicity_column: usize,
}

pub struct LookupCheckVars<F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) local_values: Vec<P>,
    pub(crate) next_values: Vec<P>,
    pub(crate) challenges: Vec<F>,
}

impl Lookup {
    pub(crate) fn eval<F, FE, P, S, const D: usize, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
        lookup_vars: &LookupCheckVars<F, FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        F: RichField + Extendable<D>,
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
        S: Stark<F, D>, {
        for challenge in &lookup_vars.challenges {
            let fe_challenge = FE::from_basefield(*challenge);

            for (i, _) in self.looking_columns.iter().enumerate() {
                let mut x = lookup_vars.local_values[i];

                x *= vars.local_values[i] + fe_challenge;

                yield_constr.constraint(x - P::ONES);
            }

            let num_helper_columns = self.num_helper_columns();
            // Check that the penultimate helper column contains `1/(table+challenge)`.
            let x = lookup_vars.local_values[num_helper_columns - 2];
            let x = x * (vars.local_values[self.looked_column] + fe_challenge);
            yield_constr.constraint(x - P::ONES);

            // Check the `Z` polynomial.
            let z = lookup_vars.local_values[num_helper_columns - 1];
            let next_z = lookup_vars.next_values[num_helper_columns - 1];
            let y = lookup_vars.local_values[..num_helper_columns - 2]
                .iter()
                .fold(P::ZEROS, |acc, x| acc + *x)
                - vars.local_values[self.multiplicity_column]
                    * lookup_vars.local_values[num_helper_columns - 2];
            yield_constr.constraint(next_z - z - y);
        }
    }

    /// This is the h(x) within the paper.
    /// Aside from the number of columns, we need:
    /// 1 column for multiplicity, and
    /// another column for the running sum.
    pub(crate) fn num_helper_columns(&self) -> usize { self.looking_columns.len() + 2 }

    /// Compute helper columns for the lookup argument.
    pub(crate) fn populate_helper_columns<F: Field>(
        &self,
        trace_poly_values: &[PolynomialValues<F>],
        challenge: F,
    ) -> Vec<PolynomialValues<F>> {
        let num_helper_columns = self.num_helper_columns();
        let mut helper_columns: Vec<PolynomialValues<F>> = Vec::with_capacity(num_helper_columns);

        for col in self.looking_columns.iter() {
            let mut column = trace_poly_values[*col].values.clone();
            for x in column.iter_mut() {
                *x = challenge + *x;
            }

            helper_columns.push(F::batch_multiplicative_inverse(&column).into());
        }

        let mut looked = trace_poly_values[self.looked_column].values.clone();
        for x in looked.iter_mut() {
            *x = challenge + *x;
        }
        helper_columns.push(F::batch_multiplicative_inverse(&looked).into());

        let multiplicities = &trace_poly_values[self.multiplicity_column].values;
        let mut z = Vec::with_capacity(multiplicities.len());
        z.push(F::ZERO);
        for i in 0..multiplicities.len() - 1 {
            let x = helper_columns[..num_helper_columns - 2]
                .iter()
                .map(|col| col.values[i])
                .sum::<F>()
                - multiplicities[i] * helper_columns.last().unwrap().values[i];
            z.push(z[i] + x);
        }
        helper_columns.push(z.into());

        helper_columns
    }
}
