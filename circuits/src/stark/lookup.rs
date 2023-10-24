use itertools::izip;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;

use super::mozak_stark::{TableKind, NUM_TABLES};
use super::permutation::challenge::GrandProductChallengeSet;
use super::proof::StarkProof;
use crate::{rangecheck, rangecheck_limb};

#[derive(Debug, Clone)]
pub struct Lookup {
    pub(crate) looking_columns: Vec<usize>,
    pub(crate) looked_column: usize,
    pub(crate) multiplicity_column: usize,
}

#[derive(Debug, Clone)]
pub struct Looking {
    pub kind: TableKind,
    pub columns: Vec<usize>,
}

#[allow(unused)]
#[derive(Clone, Debug)]
pub struct CrossTableLogup {
    pub looking_tables: Vec<Looking>,
    pub looked_table: Looked,
}

#[derive(Debug, Clone)]
pub struct Looked {
    pub kind: TableKind,
    /// t(x)
    pub table_column: usize,
    /// m(x)
    pub multiplicity_column: usize,
}

#[must_use]
pub fn rangechecks_u32() -> CrossTableLogup {
    CrossTableLogup {
        looking_tables: vec![Looking {
            kind: TableKind::RangeCheck,
            columns: rangecheck::columns::MAP.limbs.to_vec(),
        }],
        looked_table: Looked {
            kind: TableKind::RangeCheckLimb,
            table_column: rangecheck_limb::columns::MAP.logup_u8.value,
            multiplicity_column: rangecheck_limb::columns::MAP.logup_u8.multiplicity,
        },
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct LookupCheckVars<F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) local_values: Vec<P>,
    pub(crate) next_values: Vec<P>,
    pub(crate) challenges: Vec<F>,
}

impl<
        F: Field,
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
        const D2: usize,
    > LookupCheckVars<F, FE, P, D2>
{
    pub(crate) fn is_empty(&self) -> bool { self.challenges.len() == 0 }
}

pub struct LogupCheckVars<F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) looking_vars: LookupCheckVars<F, FE, P, D2>,
    pub(crate) looked_vars: LookupCheckVars<F, FE, P, D2>,
}

impl<'a, F: RichField + Extendable<D>, const D: usize>
    LogupCheckVars<F, F::Extension, F::Extension, D>
{
    pub(crate) fn from_proofs<C: GenericConfig<D, F = F>>(
        proofs: &[StarkProof<F, C, D>; NUM_TABLES],
        cross_table_logups: &'a [CrossTableLogup],
        ctl_challenges: &'a GrandProductChallengeSet<F>,
    ) -> Vec<Self> {
        let mut num_looking_per_table = [0; NUM_TABLES];
        let mut num_looked_per_table = [0; NUM_TABLES];

        for _ in &ctl_challenges.challenges {
            for logup in cross_table_logups {
                for looking_table in &logup.looking_tables {
                    println!("LTL: {}", looking_table.columns.len());
                    num_looking_per_table[looking_table.kind as usize] +=
                        looking_table.columns.len();
                }

                num_looked_per_table[logup.looked_table.kind as usize] += 1;
            }
        }

        let challenges = ctl_challenges
            .challenges
            .iter()
            .map(|c| c.beta)
            .collect::<Vec<_>>();

        let mut logup_check_vars_per_table = Vec::with_capacity(NUM_TABLES);
        for (i, p) in proofs.iter().enumerate() {
            let openings = &p.openings;

            let num_looking = num_looking_per_table[i];
            let num_looked = num_looked_per_table[i];

            println!(
                "{i} NLOOKING = {:?}, NLOOKED = {:?}",
                num_looking, num_looked
            );
            logup_check_vars_per_table.push(LogupCheckVars {
                looking_vars: LookupCheckVars {
                    local_values: openings.aux_polys[..num_looking].to_vec(),
                    next_values: openings.aux_polys_next[..num_looking].to_vec(),
                    challenges: challenges.clone(),
                },
                looked_vars: LookupCheckVars {
                    local_values: openings.aux_polys[num_looking..num_looking + num_looked]
                        .to_vec(),
                    next_values: openings.aux_polys_next[num_looking..num_looking + num_looked]
                        .to_vec(),
                    challenges: challenges.clone(),
                },
            });
        }

        logup_check_vars_per_table
    }
}
