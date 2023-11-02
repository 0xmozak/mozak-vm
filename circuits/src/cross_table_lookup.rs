use std::collections::VecDeque;

use anyhow::{ensure, Result};
use itertools::{izip, Itertools};
use plonky2::field::batch_util::batch_add_inplace;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::Stark;
use thiserror::Error;

pub use crate::linear_combination::Column;
use crate::stark::lookup::{CrossTableLogup, LogupCheckVars};
use crate::stark::mozak_stark::{Table, TableKind, NUM_TABLES};
use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};
use crate::stark::proof::StarkProof;

#[derive(Error, Debug)]
pub enum LookupError {
    #[error("Non-binary filter at row {0}")]
    NonBinaryFilter(usize),
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
    pub fn is_empty(&self) -> bool { self.zs_columns.len() == 0 }

    #[must_use]
    pub fn z_polys(&self) -> Vec<PolynomialValues<F>> {
        self.zs_columns
            .iter()
            .map(|zs_columns| zs_columns.z.clone())
            .collect()
    }
}

/// Cross-table lookup data associated with one Z(x) polynomial.
#[derive(Clone)]
pub(crate) struct CtlZData<F: Field> {
    pub(crate) z: PolynomialValues<F>,
    pub(crate) challenge: GrandProductChallenge<F>,
    pub(crate) columns: Vec<Column<F>>,
    pub(crate) filter_column: Column<F>,
}

/// Has a vector of columns, and a z_looking.
pub(crate) struct LookingHelpers<F: Field> {
    pub(crate) looking: Vec<PolynomialValues<F>>,
    pub(crate) looking_columns: Vec<usize>,
    pub(crate) z_looking: PolynomialValues<F>,
    /// The table kind that this set of helpers is looking into.
    pub(crate) to: TableKind,
}

/// Will always have 3 columns:
/// looked, multiplicity, z_looked.
pub(crate) struct LookedHelpers<F: Field> {
    pub(crate) looked: PolynomialValues<F>,
    pub(crate) looked_column: usize,
    /// m(x)
    pub(crate) multiplicities: PolynomialValues<F>,
    pub(crate) multiplicity_column: usize,
    /// A z column in a LogupHelper either contains:
    ///
    /// 1 / (X + f(x)), or
    /// m(x) / X + t(x)),
    ///
    /// depending on if they are a looking or a looked set of
    /// helper columns.
    pub(crate) z_looked: PolynomialValues<F>,
    /// The table kind that this set of helpers is looked from.
    pub(crate) from: Vec<TableKind>,
}

/// Cross-table logup data associated with one table.
///
/// A struct of `LogupHelpers` can either have only 1 of looking or looked
/// helpers, or both.
///
/// Note that this struct is cross-table in nature, i.e. the looking
/// and looked can refer to values from different tables.
/// Eventually we want to constrain the running sum using this,
/// i.e.
/// Z(i+1) = Z(i) + f_i / (challenge + a_i) (looking),
/// Z(i+1) = Z(i) + m_i / (challenge + b_i) (looked).
pub(crate) struct LogupHelpers<F: Field> {
    pub(crate) looking_helpers: Vec<LookingHelpers<F>>,
    pub(crate) looked_helpers: Vec<LookedHelpers<F>>,
}

impl<F: Field> LogupHelpers<F> {
    /// Get total number of helper columns in a table.
    ///
    /// For looking tables:
    ///   (a variable number of looking columns, + z_looking) * num_tables
    /// For looked tables:
    ///   looked column, multiplicity column, z_looking, i.e. 3 * num_tables
    pub(crate) fn total_num_columns(&self) -> usize {
        self.looking_helpers
            .map_or(0, |lhs| lhs.iter().map(|h| h.looking.len() + 1).sum())
            + self.looked_helpers.map_or(0, |lhs| lhs.len() * 3)
    }
}

pub(crate) fn verify_cross_table_lookups<F: RichField + Extendable<D>, const D: usize>(
    cross_table_lookups: &[CrossTableLookup<F>],
    ctl_zs_lasts: &[Vec<F>; NUM_TABLES],
    config: &StarkConfig,
) -> Result<()> {
    let mut ctl_zs_openings = ctl_zs_lasts.iter().map(|v| v.iter()).collect::<Vec<_>>();
    for CrossTableLookup {
        looking_tables,
        looked_table,
    } in cross_table_lookups
    {
        for _ in 0..config.num_challenges {
            let looking_zs_prod = looking_tables
                .iter()
                .map(|table| *ctl_zs_openings[table.kind as usize].next().unwrap())
                .product::<F>();
            let looked_z = *ctl_zs_openings[looked_table.kind as usize].next().unwrap();

            ensure!(
                looking_zs_prod == looked_z,
                "Cross-table lookup verification failed for {:?}->{:?} ({} != {})",
                looking_tables[0].kind,
                looked_table.kind,
                looking_zs_prod,
                looked_z
            );
        }
    }
    debug_assert!(ctl_zs_openings.iter_mut().all(|iter| iter.next().is_none()));

    Ok(())
}

pub(crate) fn verify_cross_table_logups<F, C, FE, P, const D: usize, const D2: usize>(
    cross_table_logups: &[CrossTableLogup],
    config: &StarkConfig,
    proofs: &[StarkProof<F, C, D>],
    logup_vars: &[LogupCheckVars<F, FE, P, D2>],
) -> Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    let aux_polys_per_table = proofs
        .iter()
        .map(|p| p.openings.aux_polys.into_iter())
        .collect::<Vec<_>>();
    let aux_polys_next_per_table = proofs
        .iter()
        .map(|p| p.openings.aux_polys_next)
        .collect::<Vec<_>>();

    let mut chunks_per_table = [0; NUM_TABLES].map(|_| VecDeque::new());
    // First find out how much to advance by for each aux poly and
    // aux poly next per table.
    for CrossTableLogup {
        looking_tables,
        looked_table,
    } in cross_table_logups
    {
        for looking_table in looking_tables {
            chunks_per_table[looking_table.kind as usize]
                .push_back(looking_table.columns.len() + 1);
        }

        // Always 3 - table, multiplicity, z_looked.
        chunks_per_table[looked_table.kind as usize].push_back(3);
    }

    for CrossTableLogup {
        looking_tables,
        looked_table,
    } in cross_table_logups
    {
        let mut looking_sums: F::Extension = F::ZERO.into();

        for looking_table in looking_tables {
            let chunk_len = chunks_per_table[looking_table.kind as usize]
                .pop_front()
                .unwrap();
            let aux_polys = aux_polys_per_table[looking_table.kind as usize];

            let z_looking = aux_polys.take(chunk_len).next().unwrap();

            looking_sums += z_looking;
        }
        // Assert that looking

        let chunk_len = chunks_per_table[looked_table.kind as usize]
            .pop_front()
            .unwrap();
        let looked_sum = aux_polys_per_table[looked_table.kind as usize]
            .take(chunk_len)
            .into_iter()
            .sum::<F::Extension>();

        ensure!(
            looking_sums == looked_sum,
            "Sumcheck failed between {:?} tables",
            looked_table.kind,
        );
    }
    assert!(
        chunks_per_table.into_iter().all(|c| c.is_empty()),
        "Some chunks weren't taken"
    );

    Ok(())
}

/// Builds the helper columns per table for all declared [`CrossTableLogup`]s.
///
/// Since this is cross table, some tables will have one of either looking or
/// looked columns, and some will have both.
pub(crate) fn cross_table_helper_columns<F: RichField + Extendable<D>, const D: usize>(
    all_trace_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    lookups: &[CrossTableLogup],
    challenges: &[F],
) -> [LogupHelpers<F>; NUM_TABLES] {
    /// Adds challenge to each entry in the column.
    fn log_derivative<F: Field>(mut column: Vec<F>, challenge: F) -> PolynomialValues<F> {
        for x in &mut column {
            *x = challenge + *x;
        }

        PolynomialValues::from(F::batch_multiplicative_inverse(&column))
    }

    let mut helpers_per_table: [LogupHelpers<F>; NUM_TABLES] =
        [0; NUM_TABLES].map(|_| LogupHelpers {
            looking_helpers: vec![],
            looked_helpers: vec![],
        });

    for CrossTableLogup {
        looking_tables,
        looked_table,
    } in lookups
    {
        let mut looking_columns = Vec::with_capacity(looking_tables.len());
        for challenge in challenges {
            // Calculate all helper columns for looked.
            let looked_poly_values = &all_trace_poly_values[looked_table.kind as usize];
            let multiplicities = looked_poly_values[looked_table.multiplicity_column].clone();
            let table_column = looked_poly_values[looked_table.table_column].values.clone();

            let mut z_looked = Vec::with_capacity(multiplicities.len());
            z_looked.push(F::ZERO);

            for looking_table in looking_tables {
                let mut z_looking = Vec::with_capacity(multiplicities.len());
                z_looking.push(F::ZERO);
                let mut looking = Vec::new();
                let mut looking_indices = Vec::new();

                let looking_poly_values = &all_trace_poly_values[looking_table.kind as usize];

                // Calculates 1 / x + f(x), which prepares the column to be constrained as per
                // Lemma 5 within the LogUp paper.

                // Calculate all helper columns for looking.
                for col in &looking_table.columns {
                    let column_inverse =
                        log_derivative(looking_poly_values[*col].values.clone(), *challenge);
                    looking.push(column_inverse);
                    looking_indices.push(*col);
                }

                // sum(1 / (x + f(x)) for all looking columns.
                for i in 0..multiplicities.len() - 1 {
                    let looking_x = looking.iter().map(|c| c.values[i]).sum::<F>();
                    z_looking.push(z_looking[i] + looking_x);
                }

                let looking_helpers = LookingHelpers {
                    looking,
                    looking_columns,
                    z_looking: z_looking.into(),
                    to: looked_table.kind,
                };
                helpers_per_table[looking_table.kind as usize]
                    .looking_helpers
                    .push(looking_helpers);
            }

            // Calculates 1 / x + t(x), leaving out the m(x) to be multiplied in later.
            let table_inverse = log_derivative(table_column, *challenge);

            for i in 0..multiplicities.len() - 1 {
                table_inverse
                    .values
                    .iter()
                    .for_each(|c| z_looked.push(z_looked[i] + *c * multiplicities.values[i]));
            }

            let looked_helpers = LookedHelpers {
                looked: table_column.into(),
                looked_column: looked_table.table_column,
                multiplicities,
                multiplicity_column: looked_table.multiplicity_column,
                z_looked: z_looked.into(),
                from: looking_tables.iter().map(|t| t.kind).collect::<Vec<_>>(),
            };
            helpers_per_table[looked_table.kind as usize]
                .looked_helpers
                .push(looked_helpers);
        }
    }

    helpers_per_table
}

pub(crate) fn cross_table_lookup_data<F: RichField, const D: usize>(
    trace_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    cross_table_lookups: &[CrossTableLookup<F>],
    ctl_challenges: &GrandProductChallengeSet<F>,
) -> [CtlData<F>; NUM_TABLES] {
    let mut ctl_data_per_table = [0; NUM_TABLES].map(|_| CtlData::default());
    for CrossTableLookup {
        looking_tables,
        looked_table,
    } in cross_table_lookups
    {
        log::debug!("Processing CTL for {:?}", looked_table.kind);
        for &challenge in &ctl_challenges.challenges {
            let zs_looking = looking_tables.iter().map(|looking_table| {
                partial_products(
                    &trace_poly_values[looking_table.kind as usize],
                    &looking_table.columns,
                    &looking_table.filter_column,
                    challenge,
                )
            });
            let z_looked = partial_products(
                &trace_poly_values[looked_table.kind as usize],
                &looked_table.columns,
                &looked_table.filter_column,
                challenge,
            );

            debug_assert_eq!(
                zs_looking
                    .clone()
                    .map(|z| *z.values.last().unwrap())
                    .product::<F>(),
                *z_looked.values.last().unwrap()
            );

            for (looking_table, z) in looking_tables.iter().zip(zs_looking) {
                ctl_data_per_table[looking_table.kind as usize]
                    .zs_columns
                    .push(CtlZData {
                        z,
                        challenge,
                        columns: looking_table.columns.clone(),
                        filter_column: looking_table.filter_column.clone(),
                    });
            }
            ctl_data_per_table[looked_table.kind as usize]
                .zs_columns
                .push(CtlZData {
                    z: z_looked,
                    challenge,
                    columns: looked_table.columns.clone(),
                    filter_column: looked_table.filter_column.clone(),
                });
        }
    }
    ctl_data_per_table
}

fn partial_products<F: Field>(
    trace: &[PolynomialValues<F>],
    columns: &[Column<F>],
    filter_column: &Column<F>,
    challenge: GrandProductChallenge<F>,
) -> PolynomialValues<F> {
    // design of table looks like this
    //       |  filter  |   value   |  partial_prod |
    //       |    1     |    x_1    |  x_3          |
    //       |    0     |    x_2    |  x_3 * x_1    |
    //       |    1     |    x_3    |  x_3 * x_1    |
    // this is done so that now transition constraint looks like
    //       z_next = z_local * select(value_local, filter_local)
    // That is, there is no need for reconstruction of value_next.
    // In current design which uses lv and nv values from columns to construct the
    // final value_local, its impossible to construct value_next from lv and nv
    // values of current row

    let combine_if_filter_at_i = |i| -> F {
        let filter = filter_column.eval_table(trace, i);
        if filter.is_one() {
            let evals = columns
                .iter()
                .map(|c| c.eval_table(trace, i))
                .collect::<Vec<_>>();
            challenge.combine(evals.iter())
        } else {
            assert_eq!(filter, F::ZERO, "Non-binary filter?");
            F::ONE
        }
    };

    let degree = trace[0].len();
    let mut degrees = (0..degree).collect::<Vec<_>>();
    degrees.rotate_right(1);
    degrees
        .into_iter()
        .map(combine_if_filter_at_i)
        .scan(F::ONE, |partial_prod: &mut F, combined| {
            *partial_prod *= combined;
            Some(*partial_prod)
        })
        .collect_vec()
        .into()
}

#[allow(unused)]
#[derive(Clone, Debug)]
pub struct CrossTableLookup<F: Field> {
    pub looking_tables: Vec<Table<F>>,
    pub looked_table: Table<F>,
}

impl<F: Field> CrossTableLookup<F> {
    /// Instantiates a new cross table lookup between 2 tables.
    ///
    /// # Panics
    /// Panics if the two tables do not have equal number of columns.
    pub fn new(looking_tables: Vec<Table<F>>, looked_table: Table<F>) -> Self {
        assert!(looking_tables
            .iter()
            .all(|twc| twc.columns.len() == looked_table.columns.len()));
        Self {
            looking_tables,
            looked_table,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CtlCheckVars<'a, F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) local_z: P,
    pub(crate) next_z: P,
    pub(crate) challenges: GrandProductChallenge<F>,
    pub(crate) columns: &'a [Column<F>],
    pub(crate) filter_column: &'a Column<F>,
}

impl<'a, F: RichField + Extendable<D>, const D: usize>
    CtlCheckVars<'a, F, F::Extension, F::Extension, D>
{
    pub(crate) fn from_proofs<C: GenericConfig<D, F = F>>(
        proofs: &[StarkProof<F, C, D>; NUM_TABLES],
        cross_table_lookups: &'a [CrossTableLookup<F>],
        ctl_challenges: &'a GrandProductChallengeSet<F>,
        num_logups_per_table: &'a [usize; NUM_TABLES],
    ) -> [Vec<Self>; NUM_TABLES] {
        println!("NLPT: {:?}", num_logups_per_table);
        let mut ctl_zs = proofs
            .iter()
            .zip(num_logups_per_table)
            .map(|(p, &num)| {
                // skip looking and looked
                let openings = &p.openings;
                let ctl_zs = openings.aux_polys.iter().skip(num);
                let ctl_zs_next = openings.aux_polys_next.iter().skip(num);
                ctl_zs.zip(ctl_zs_next)
            })
            .collect::<Vec<_>>();

        let mut ctl_vars_per_table = [0; NUM_TABLES].map(|_| vec![]);
        for CrossTableLookup {
            looking_tables,
            looked_table,
        } in cross_table_lookups
        {
            for &challenges in &ctl_challenges.challenges {
                for table in looking_tables {
                    let (looking_z, looking_z_next) = ctl_zs[table.kind as usize].next().unwrap();
                    ctl_vars_per_table[table.kind as usize].push(Self {
                        local_z: *looking_z,
                        next_z: *looking_z_next,
                        challenges,
                        columns: &table.columns,
                        filter_column: &table.filter_column,
                    });
                }

                let (looked_z, looked_z_next) = ctl_zs[looked_table.kind as usize].next().unwrap();
                ctl_vars_per_table[looked_table.kind as usize].push(Self {
                    local_z: *looked_z,
                    next_z: *looked_z_next,
                    challenges,
                    columns: &looked_table.columns,
                    filter_column: &looked_table.filter_column,
                });
            }
        }
        for var in &ctl_vars_per_table {
            println!("ctl vars per table {:?}", var.len());
        }
        ctl_vars_per_table
    }
}

pub(crate) fn eval_cross_table_logup<F, FE, P, S, const D: usize, const D2: usize>(
    vars: &S::EvaluationFrame<FE, P, D2>,
    logup_vars: &LogupCheckVars<F, FE, P, D2>,
    challenges: &[F],
    yield_constr: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>, {
    if logup_vars.is_empty() {
        return;
    };

    let lvs: &[P] = vars.get_local_values();

    // For looking vars, we need to ensure:
    //
    // 1) All inverse columns are well-formed,
    // 2) z_looking_i+1 = z_looking_i + 1 / (X + f_i)
    if !logup_vars.looking_vars.is_empty() {
        let lvs_to_check = &logup_vars.looking_vars.local_values;
        let columns = &logup_vars.looking_vars.columns;

        let chunk_len = lvs_to_check.len() / challenges.len();
        for (i, (c, lv)) in izip!(columns, lvs_to_check).enumerate() {
            let challenge = challenges.get(i / chunk_len).unwrap();
            let challenge = FE::from_basefield(*challenge);
            yield_constr.constraint(
                (lvs[usize::try_from(c.to_canonical_u64())
                    .expect("cast from u64 to usize should succeed")]
                    + challenge)
                    * *lv
                    - P::ONES,
            );
        }
    }

    // For looked vars, we need to ensure:
    //
    // 1) All inverse columns are well-formed,
    // 2) z_looked_i+1 = z_looked_i + m_i / (X + f_i)
    if !logup_vars.looked_vars.is_empty() {
        let lvs_to_check = &logup_vars.looked_vars.local_values;
        let columns = &logup_vars.looked_vars.columns;

        let chunk_len = lvs_to_check.len() / challenges.len();
        for (i, (c, lv)) in izip!(columns, lvs_to_check).enumerate() {
            let challenge = challenges.get(i / chunk_len).unwrap();
            let challenge = FE::from_basefield(*challenge);
            yield_constr.constraint(
                (lvs[usize::try_from(c.to_canonical_u64())
                    .expect("cast from u64 to usize should succeed")]
                    + challenge)
                    * *lv
                    - P::ONES,
            );
        }
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
        let filter = |lv: &[P], nv: &[P]| -> P { filter_column.eval(lv, nv) };
        let filter = filter(local_values, next_values);
        let select = |filter, x| filter * x + P::ONES - filter;

        // Check value of `Z(1)`
        consumer.constraint_last_row(*next_z - select(filter, combination));
        // Check `Z(gw) = combination * Z(w)`
        consumer.constraint_transition(*next_z - *local_z * select(filter, combination));
    }
}

pub mod ctl_utils {
    use std::collections::HashMap;
    use std::ops::{Deref, DerefMut};

    use plonky2::field::extension::Extendable;
    use plonky2::field::polynomial::PolynomialValues;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::RichField;

    use crate::cross_table_lookup::{CrossTableLookup, LookupError};
    use crate::stark::mozak_stark::{MozakStark, Table, TableKind, NUM_TABLES};

    struct MultiSet<F>(HashMap<Vec<F>, Vec<(TableKind, usize)>>);

    impl<F: Field> Deref for MultiSet<F> {
        type Target = HashMap<Vec<F>, Vec<(TableKind, usize)>>;

        fn deref(&self) -> &Self::Target { &self.0 }
    }
    impl<F: Field> DerefMut for MultiSet<F> {
        fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
    }
    impl<F: Field> MultiSet<F> {
        pub fn new() -> Self { MultiSet(HashMap::new()) }

        fn process_row(
            &mut self,
            trace_poly_values: &[Vec<PolynomialValues<F>>],
            table: &Table<F>,
        ) -> Result<(), LookupError> {
            let trace = &trace_poly_values[table.kind as usize];
            for i in 0..trace[0].len() {
                let filter = table.filter_column.eval_table(trace, i);
                if filter.is_one() {
                    let row = table
                        .columns
                        .iter()
                        .map(|c| c.eval_table(trace, i))
                        .collect::<Vec<_>>();
                    self.entry(row).or_default().push((table.kind, i));
                } else if !filter.is_zero() {
                    return Err(LookupError::NonBinaryFilter(i));
                }
            }

            Ok(())
        }
    }

    pub fn check_single_ctl<F: Field>(
        trace_poly_values: &[Vec<PolynomialValues<F>>],
        ctl: &CrossTableLookup<F>,
    ) -> Result<(), LookupError> {
        // Maps `m` with `(table.kind, i) in m[row]` iff the `i`-th row of the table
        // is equal to `row` and the filter is 1.
        //
        // the CTL check holds iff `looking_multiset == looked_multiset`.
        let mut looking_multiset = MultiSet::<F>::new();
        let mut looked_multiset = MultiSet::<F>::new();

        for looking_table in &ctl.looking_tables {
            looking_multiset.process_row(trace_poly_values, looking_table)?;
        }

        looked_multiset.process_row(trace_poly_values, &ctl.looked_table)?;
        let empty = &vec![];

        // Check that every row in the looking tables appears in the looked table the
        // same number of times.
        for (row, looking_locations) in &looking_multiset.0 {
            let looked_locations = looked_multiset.get(row).unwrap_or(empty);
            if looking_locations.len() != looked_locations.len() {
                println!(
                    "Row {row:?} is present {l0} times in the looking tables, but
                    {l1} times in the looked table.\n\
                    Looking locations: {looking_locations:?}.\n\
                    Looked locations: {looked_locations:?}.",
                    l0 = looking_locations.len(),
                    l1 = looked_locations.len()
                );
                return Err(LookupError::InconsistentTableRows);
            }
        }

        // Check that every row in the looked tables appears in the looking table the
        // same number of times.
        for (row, looked_locations) in &looked_multiset.0 {
            let looking_locations = looking_multiset.get(row).unwrap_or(empty);
            if looking_locations.len() != looked_locations.len() {
                println!(
                    "Row {row:?} is present {l0} times in the looking tables, but
                    {l1} times in the looked table.\n\
                    Looking locations: {looking_locations:?}.\n\
                    Looked locations: {looked_locations:?}.",
                    l0 = looking_locations.len(),
                    l1 = looked_locations.len()
                );
                return Err(LookupError::InconsistentTableRows);
            }
        }

        Ok(())
    }
    pub fn debug_ctl<F: RichField + Extendable<D>, const D: usize>(
        traces_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
        mozak_stark: &MozakStark<F, D>,
    ) {
        mozak_stark
            .cross_table_lookups
            .iter()
            .for_each(|ctl| check_single_ctl(traces_poly_values, ctl).unwrap());
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use itertools::Itertools;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::polynomial::PolynomialValues;

    use super::ctl_utils::check_single_ctl;
    use super::*;
    use crate::stark::mozak_stark::{CpuTable, Lookups, RangeCheckTable};

    #[allow(clippy::similar_names)]
    /// Specify which column(s) to find data related to lookups.
    /// If the lengths of `lv_col_indices` and `nv_col_indices` are not same,
    /// then we resize smaller one with empty column and then add componentwise
    fn lookup_data<F: Field>(lv_col_indices: &[usize], nv_col_indices: &[usize]) -> Vec<Column<F>> {
        // use usual lv values of the rows
        let lv_columns = Column::singles(lv_col_indices);
        // use nv values of the rows
        let nv_columns = Column::singles_next(nv_col_indices);

        lv_columns
            .into_iter()
            .zip_longest(nv_columns)
            .map(|item| item.reduce(std::ops::Add::add))
            .collect()
    }

    /// Specify the column index of the filter column used in lookups.
    fn lookup_filter<F: Field>(col_idx: usize) -> Column<F> { Column::single(col_idx) }

    /// A generic cross lookup table.
    struct FooBarTable<F: Field>(CrossTableLookup<F>);

    impl<F: Field> Lookups<F> for FooBarTable<F> {
        /// We use the [`CpuTable`] and the [`RangeCheckTable`] to build a
        /// [`CrossTableLookup`] here, but in principle this is meant to
        /// be used generically for tests.
        fn lookups() -> CrossTableLookup<F> {
            CrossTableLookup {
                looking_tables: vec![CpuTable::new(lookup_data(&[1], &[2]), lookup_filter(0))],
                looked_table: RangeCheckTable::new(lookup_data(&[1], &[]), lookup_filter(0)),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Trace<F: Field> {
        trace: Vec<PolynomialValues<F>>,
    }

    #[derive(Default)]
    pub struct TraceBuilder<F: Field> {
        trace: Vec<PolynomialValues<F>>,
    }

    impl<F: Field> TraceBuilder<F> {
        /// Creates a new trace with the given `num_cols` and `num_rows`.
        pub fn new(num_cols: usize, num_rows: usize) -> TraceBuilder<F> {
            let mut trace = vec![];
            for _ in 0..num_cols {
                let mut values = Vec::with_capacity(num_rows);
                for _ in 0..num_rows {
                    values.push(F::rand());
                }
                trace.push(PolynomialValues::from(values));
            }

            TraceBuilder { trace }
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// zeroes.
        #[allow(unused)]
        pub fn zero(mut self, idx: usize) -> TraceBuilder<F> {
            self.trace[idx] = PolynomialValues::zero(self.trace[idx].len());

            self
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// `F::ONE`.
        pub fn one(mut self, col_idx: usize) -> TraceBuilder<F> {
            let len = self.trace[col_idx].len();
            let ones = PolynomialValues::constant(F::ONE, len);
            self.trace[col_idx] = ones;

            self
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// `value`. This is convenient for testing cross table lookups.
        pub fn set_values(mut self, col_idx: usize, value: usize) -> TraceBuilder<F> {
            let len = self.trace[col_idx].len();
            let new_v: Vec<F> = (0..len).map(|_| F::from_canonical_usize(value)).collect();
            let values = PolynomialValues::from(new_v);
            self.trace[col_idx] = values;

            self
        }

        /// Set all polynomial values at a given column index `col_idx` to
        /// alternate between `value_1` and `value_2`. Useful for testing
        /// combination of lv and nv values
        pub fn set_values_alternate(
            mut self,
            col_idx: usize,
            value_1: usize,
            value_2: usize,
        ) -> TraceBuilder<F> {
            let len = self.trace[col_idx].len();
            self.trace[col_idx] = PolynomialValues::from(
                [value_1, value_2]
                    .into_iter()
                    .cycle()
                    .take(len)
                    .map(F::from_canonical_usize)
                    .collect_vec(),
            );

            self
        }

        pub fn build(self) -> Vec<PolynomialValues<F>> { self.trace }
    }
    /// A generic cross lookup table.
    struct NonBinaryFilterTable<F: Field>(CrossTableLookup<F>);

    impl<F: Field> Lookups<F> for NonBinaryFilterTable<F> {
        /// We use the [`CpuTable`] and the [`RangeCheckTable`] to build a
        /// [`CrossTableLookup`] here, but in principle this is meant to
        /// be used generically for tests.
        fn lookups() -> CrossTableLookup<F> {
            CrossTableLookup {
                looking_tables: vec![CpuTable::new(lookup_data(&[1], &[2]), lookup_filter(0))],
                looked_table: RangeCheckTable::new(lookup_data(&[1], &[]), lookup_filter(0)),
            }
        }
    }

    /// Create a table with a filter column that's non-binary, which should
    /// cause our manual checks to fail.
    #[test]
    fn test_ctl_non_binary_filters() {
        type F = GoldilocksField;

        let dummy_cross_table_lookup: CrossTableLookup<F> = NonBinaryFilterTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(1).set_values(1, 5).build(); // filter column is random
        let bar_trace: Vec<PolynomialValues<F>> =
            TraceBuilder::new(3, 4).one(0).set_values(1, 5).build();
        let traces = vec![foo_trace, bar_trace];
        assert!(matches!(
            check_single_ctl(&traces, &dummy_cross_table_lookup).unwrap_err(),
            LookupError::NonBinaryFilter(0)
        ));
    }

    /// Create a trace with inconsistent values, which should
    /// cause our manual checks to fail.
    /// Here, `foo_trace` has all values in column 1 and 2 set to alternate
    /// between 2 and 3 while `bar_trace` has all values in column 1 set to
    /// 6. Since lookup data is sum of lv values of column 1 and nv values
    /// of column 2 from `foo_trace`, our manual checks will fail this test.
    #[test]
    fn test_ctl_inconsistent_tables() {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup<F> = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values_alternate(1, 2, 3)
            .set_values_alternate(2, 2, 3)
            .build();
        let bar_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values(1, 6)
            .build();
        let traces = vec![foo_trace, bar_trace];
        assert!(matches!(
            check_single_ctl(&traces, &dummy_cross_table_lookup).unwrap_err(),
            LookupError::InconsistentTableRows
        ));
    }

    /// Happy path test where all checks go as plan.
    /// Here, `foo_trace` has all values in column 1 set to alternate between 2
    /// and 3, and values in column 2 set to alternate between 3 and 2 while
    /// `bar_trace` has all values in column 1 set to 5. Since lookup data
    /// is sum of lv values of column 1 and nv values of column 2 from
    /// `foo_trace`, our manual checks will pass the test
    #[test]
    fn test_ctl() -> Result<()> {
        type F = GoldilocksField;
        let dummy_cross_table_lookup: CrossTableLookup<F> = FooBarTable::lookups();

        let foo_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values_alternate(1, 2, 3)
            .set_values_alternate(2, 2, 3)
            .build();
        let bar_trace: Vec<PolynomialValues<F>> = TraceBuilder::new(3, 4)
            .one(0) // filter column
            .set_values(1, 5)
            .build();
        let traces = vec![foo_trace, bar_trace];
        check_single_ctl(&traces, &dummy_cross_table_lookup)?;
        Ok(())
    }
}
