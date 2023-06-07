use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::fri::structure::{FriBatchInfo, FriInstanceInfo, FriOracleInfo, FriPolynomialInfo};
use plonky2::{field::extension::Extendable, hash::hash_types::RichField};

use super::config::StarkConfig;
use super::constraint_consumer::ConstraintConsumer;
use super::vars::StarkEvaluationVars;

const TRACE_ORACLE_INDEX: usize = 0;
const QUOTIENT_ORACLE_INDEX: usize = 1;

/// A STARK System.
pub trait Stark<F: RichField + Extendable<D>, const D: usize>: Sync {
    /// The total number of columns in the trace.
    const COLUMNS: usize;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;

    fn constraint_degree(&self) -> usize;
    fn quotient_degree_factor(&self) -> usize {
        1.max(self.constraint_degree() - 1)
    }
    fn num_quotient_poly(&self, config: &StarkConfig) -> usize {
        self.quotient_degree_factor() * config.num_challenges
    }

    /// Computes the FRI instance used to prove this Stark.
    fn fri_instance(
        &self,
        zeta: F::Extension,
        g: F,
        config: &StarkConfig,
    ) -> FriInstanceInfo<F, D> {
        let trace_oracle = FriOracleInfo {
            num_polys: Self::COLUMNS,
            blinding: false,
        };
        let trace_info = FriPolynomialInfo::from_range(TRACE_ORACLE_INDEX, 0..Self::COLUMNS);

        let num_quotient_polys = self.num_quotient_poly(config);
        let quotient_oracle = FriOracleInfo {
            num_polys: num_quotient_polys,
            blinding: false,
        };
        let quotient_info =
            FriPolynomialInfo::from_range(QUOTIENT_ORACLE_INDEX, 0..num_quotient_polys);

        let zeta_batch = FriBatchInfo {
            point: zeta,
            polynomials: [trace_info.clone(), quotient_info].concat(),
        };
        let zeta_next_batch = FriBatchInfo {
            point: zeta.scalar_mul(g),
            polynomials: [trace_info].concat(),
        };
        FriInstanceInfo {
            oracles: vec![trace_oracle, quotient_oracle],
            batches: vec![zeta_batch, zeta_next_batch],
        }
    }
}
