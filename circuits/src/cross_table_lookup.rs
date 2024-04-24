use core::ops::Neg;

use anyhow::{ensure, Result};
use itertools::{iproduct, izip, zip_eq};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::cross_table_lookup as starky_ctl;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::Stark;
use thiserror::Error;

pub use crate::linear_combination::Column;
use crate::linear_combination::ColumnSparse;
pub use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::stark::mozak_stark::{all_kind, Table, TableKind, TableKindArray, TableWithTypedOutput};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};
use crate::stark::proof::{StarkProof, StarkProofTarget};

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("Inconsistency found between looking and looked tables")]
    InconsistentTableRows,
}

#[derive(Clone, Default)]
pub struct CtlData<F: Field> {
    pub(crate) zs_columns: Vec<CtlZData<F>>,
}

impl<F: Field> CtlData<F> {
    #[must_use]
    pub fn len(&self) -> usize { self.zs_columns.len() }

    #[must_use]
    pub fn is_empty(&self) -> bool { self.zs_columns.is_empty() }

    #[must_use]
    pub fn z_polys(&self) -> Vec<PolynomialValues<F>> {
        self.zs_columns
            .iter()
            .map(|zs_column| zs_column.z.clone())
            .collect()
    }
}

/// Cross-table lookup data associated with one Z(x) polynomial.
#[derive(Clone)]
pub(crate) struct CtlZData<F: Field> {
    pub(crate) z: PolynomialValues<F>,
    pub(crate) challenge: GrandProductChallenge<F>,
    pub(crate) columns: Vec<Column>,
    pub(crate) filter_column: Column,
}

/// Treat CTL and the challenge as a single entity.
///
/// Logically, the CTL specifies a linear transformation, and so does the
/// challenge. This function combines the two into a single linear
/// transformation.
pub fn compose_ctl_with_challenge<F: Field>(
    columns: &[ColumnSparse<F>],
    challenge: GrandProductChallenge<F>,
) -> ColumnSparse<F> {
    columns
        .iter()
        .rev()
        .fold(ColumnSparse::default(), |acc, term| {
            acc * challenge.beta + term
        })
        + challenge.gamma
}

pub fn partial_sums<F: Field>(
    trace: &[PolynomialValues<F>],
    columns: &[Column],
    filter_column: &Column,
    challenge: GrandProductChallenge<F>,
) -> PolynomialValues<F> {
    // design of table looks like this
    //       |  multiplicity  |   value   |  partial_sum                      |
    //       |       1        |    x_1    |  1/combine(x_1)                   |
    //       |       0        |    x_2    |  1/combine(x_1)                   |
    //       |       2        |    x_3    |  1/combine(x_1) + 2/combine(x_3)  |
    // (where combine(vals) = gamma + reduced_sum(vals, beta))
    // transition constraint looks like
    //       z_next = z_local + filter_local/combine_local

    let filter_column = filter_column.to_field();
    let get_multiplicity = |&i| -> F { filter_column.eval_table(trace, i) };

    let columns: Vec<ColumnSparse<F>> = columns.iter().map(Column::to_field).collect();
    let prepped = compose_ctl_with_challenge(&columns, challenge);
    let get_data = |&i| -> F { prepped.eval_table(trace, i) };

    let degree = trace[0].len();
    let mut degrees = (0..degree).collect::<Vec<_>>();
    degrees.rotate_right(1);

    let multiplicities: Vec<F> = degrees.iter().map(get_multiplicity).collect();
    let data: Vec<F> = degrees.iter().map(get_data).collect();
    let inv_data = F::batch_multiplicative_inverse(&data);

    izip!(multiplicities, inv_data)
        .scan(F::ZERO, |partial_sum: &mut F, (multiplicity, inv)| {
            *partial_sum += multiplicity * inv;
            Some(*partial_sum)
        })
        .collect::<Vec<_>>()
        .into()
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct CrossTableLookupWithTypedOutput<Row> {
    pub looking_tables: Vec<TableWithTypedOutput<Row>>,
}

// This is a little trick, so that we can use `CrossTableLookup` as a
// constructor, but only when the type parameter Row = Vec<Column>.
// TODO(Matthias): See if we can do the same trick for `table_impl`.
#[allow(clippy::module_name_repetitions)]
pub type CrossTableLookupUntyped = CrossTableLookupWithTypedOutput<Vec<Column>>;
pub use CrossTableLookupUntyped as CrossTableLookup;

impl<F: Field> From<&CrossTableLookup> for starky_ctl::CrossTableLookup<F> {
    fn from(ctl: &CrossTableLookup) -> Self {
        starky_ctl::CrossTableLookup::new_no_looked_table(
            ctl.looking_tables.iter().map(Table::to_starky).collect(),
        )
    }
}

impl<F: Field> From<CrossTableLookup> for starky_ctl::CrossTableLookup<F> {
    fn from(ctl: CrossTableLookup) -> Self { Self::from(&ctl) }
}

impl<Row: IntoIterator<Item = Column>> CrossTableLookupWithTypedOutput<Row> {
    pub fn to_untyped_output(self) -> CrossTableLookup {
        let looking_tables = self
            .looking_tables
            .into_iter()
            .map(TableWithTypedOutput::to_untyped_output)
            .collect();
        CrossTableLookup { looking_tables }
    }
}

impl<Row> CrossTableLookupWithTypedOutput<Row> {
    /// Instantiates a new cross table lookup between 2 tables.
    ///
    /// # Panics
    /// Panics if the two tables do not have equal number of columns.
    #[must_use]
    pub fn new(
        mut looking_tables: Vec<TableWithTypedOutput<Row>>,
        looked_tables: Vec<TableWithTypedOutput<Row>>,
    ) -> Self {
        looking_tables.extend(looked_tables.into_iter().map(Neg::neg));
        Self { looking_tables }
    }

    #[must_use]
    pub fn num_ctl_zs(ctls: &[Self], table: TableKind, num_challenges: usize) -> usize {
        ctls.iter()
            .flat_map(|ctl| &ctl.looking_tables)
            .filter(|twc| twc.kind == table)
            .count()
            * num_challenges
    }
}

#[derive(Clone)]
pub struct CtlCheckVars<'a, F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) local_z: P,
    pub(crate) next_z: P,
    pub(crate) challenges: GrandProductChallenge<F>,
    pub(crate) columns: &'a [Column],
    pub(crate) filter_column: &'a Column,
}

impl<'a, F: RichField + Extendable<D>, const D: usize>
    CtlCheckVars<'a, F, F::Extension, F::Extension, D>
{
    pub(crate) fn from_proofs<C: GenericConfig<D, F = F>>(
        proofs: &TableKindArray<StarkProof<F, C, D>>,
        cross_table_lookups: &'a [CrossTableLookup],
        ctl_challenges: &'a GrandProductChallengeSet<F>,
    ) -> TableKindArray<Vec<Self>> {
        let mut ctl_zs = proofs
            .each_ref()
            .map(|p| izip!(&p.openings.ctl_zs, &p.openings.ctl_zs_next));

        let mut ctl_vars_per_table = all_kind!(|_kind| vec![]);
        let ctl_chain = cross_table_lookups
            .iter()
            .flat_map(|ctl| &ctl.looking_tables);
        for (&challenges, table) in iproduct!(&ctl_challenges.challenges, ctl_chain) {
            let (&local_z, &next_z) = ctl_zs[table.kind].next().unwrap();
            ctl_vars_per_table[table.kind].push(Self {
                local_z,
                next_z,
                challenges,
                columns: &table.columns,
                filter_column: &table.filter_column,
            });
        }
        ctl_vars_per_table
    }
}

pub(crate) fn eval_cross_table_lookup_checks<F, FE, P, S, const D: usize, const D2: usize>(
    vars: &S::EvaluationFrame<FE, P, D2>,
    ctl_vars: &[CtlCheckVars<F, FE, P, D2>],
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>, {
    for lookup_vars in ctl_vars {
        let CtlCheckVars {
            local_z,
            next_z,
            challenges,
            columns,
            filter_column,
        } = lookup_vars;
        let local_values = vars.get_local_values();
        let next_values = vars.get_next_values();

        let combine = |lv: &[P], nv: &[P]| -> P {
            let evals = columns.iter().map(|c| c.eval(lv, nv)).collect::<Vec<_>>();
            challenges.combine(evals.iter())
        };
        let combination = combine(local_values, next_values);
        let multiplicity = |lv: &[P], nv: &[P]| -> P { filter_column.eval(lv, nv) };
        let multiplicity = multiplicity(local_values, next_values);

        // Check value of `Z(1) = filter(w^(n-1))/combined(w^(n-1))`
        consumer.constraint_last_row(*next_z * combination - multiplicity);

        // Check `Z(gw) - Z(w) = filter(w)/combined(w)`
        consumer.constraint_transition((*next_z - *local_z) * combination - multiplicity);
    }
}

#[derive(Clone)]
pub struct CtlCheckVarsTarget<'a, const D: usize> {
    pub local_z: ExtensionTarget<D>,
    pub next_z: ExtensionTarget<D>,
    pub challenges: GrandProductChallenge<Target>,
    pub columns: &'a [Column],
    pub filter_column: &'a Column,
}

impl<'a, const D: usize> CtlCheckVarsTarget<'a, D> {
    #[must_use]
    pub fn from_proof(
        table: TableKind,
        proof: &StarkProofTarget<D>,
        cross_table_lookups: &'a [CrossTableLookup],
        ctl_challenges: &'a GrandProductChallengeSet<Target>,
    ) -> Vec<Self> {
        let ctl_zs = izip!(&proof.openings.ctl_zs, &proof.openings.ctl_zs_next);

        let ctl_chain = cross_table_lookups
            .iter()
            .flat_map(|ctl| ctl.looking_tables.iter().filter(|twc| twc.kind == table));
        zip_eq(ctl_zs, iproduct!(&ctl_challenges.challenges, ctl_chain))
            .map(|((&local_z, &next_z), (&challenges, table))| Self {
                local_z,
                next_z,
                challenges,
                columns: &table.columns,
                filter_column: &table.filter_column,
            })
            .collect()
    }
}

pub fn eval_cross_table_lookup_checks_circuit<
    S: Stark<F, D>,
    F: RichField + Extendable<D>,
    const D: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    vars: &S::EvaluationFrameTarget,
    ctl_vars: &[CtlCheckVarsTarget<D>],
    consumer: &mut RecursiveConstraintConsumer<F, D>,
) {
    for lookup_vars in ctl_vars {
        let CtlCheckVarsTarget {
            local_z,
            next_z,
            challenges,
            columns,
            filter_column,
        }: &CtlCheckVarsTarget<D> = lookup_vars;

        let local_values = vars.get_local_values();
        let next_values = vars.get_next_values();

        let evals: Vec<_> = columns
            .iter()
            .map(|c| c.eval_circuit(builder, local_values, next_values))
            .collect();
        let combined = challenges.combine_circuit(builder, &evals);

        let multiplicity = filter_column.eval_circuit(builder, local_values, next_values);

        // Check value of `Z(1) = filter(w^(n-1))/combined(w^(n-1))`
        let last_row = builder.mul_sub_extension(*next_z, combined, multiplicity);
        consumer.constraint_last_row(builder, last_row);

        // Check `Z(gw) - Z(w) = filter(w)/combined(w)`
        let diff = builder.sub_extension(*next_z, *local_z);
        let transition = builder.mul_sub_extension(diff, combined, multiplicity);
        consumer.constraint_transition(builder, transition);
    }
}

// TODO(Matthias): restore the tests from before https://github.com/0xmozak/mozak-vm/pull/1371
