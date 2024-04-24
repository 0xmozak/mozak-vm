use core::ops::Neg;

use itertools::{iproduct, izip, zip_eq};
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::RecursiveConstraintConsumer;
use starky::cross_table_lookup as starky_ctl;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::Stark;
use thiserror::Error;

pub use crate::linear_combination::Column;
use crate::linear_combination::ColumnSparse;
pub use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::stark::mozak_stark::{Table, TableKind, TableWithTypedOutput};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};
use crate::stark::proof::StarkProofTarget;

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("Inconsistency found between looking and looked tables")]
    InconsistentTableRows,
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
