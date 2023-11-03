use itertools::izip;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;

use super::mozak_stark::{TableKind, NUM_TABLES};
use super::permutation::challenge::GrandProductChallengeSet;
use super::proof::StarkProof;
use crate::{cpu, rangecheck, rangecheck_limb};

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
            kind: TableKind::Cpu,
            columns: vec![cpu::columns::MAP.cpu.dst_value],
        }],
        looked_table: Looked {
            kind: TableKind::RangeCheck,
            table_column: rangecheck::columns::MAP.logup_u32.value,
            multiplicity_column: rangecheck::columns::MAP.logup_u32.multiplicity,
        },
    }
}

#[must_use]
pub fn rangechecks_u8() -> CrossTableLogup {
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
    pub(crate) columns: Vec<F>,
}

impl<
        F: Field,
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
        const D2: usize,
    > LookupCheckVars<F, FE, P, D2>
{
    pub(crate) fn is_empty(&self) -> bool { self.local_values.len() == 0 }
}

pub struct LogupCheckVars<F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    pub(crate) looking_vars: Vec<LookupCheckVars<F, FE, P, D2>>,
    pub(crate) looked_vars: Vec<LookupCheckVars<F, FE, P, D2>>,
}
impl<
        F: Field,
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
        const D2: usize,
    > LogupCheckVars<F, FE, P, D2>
{
    pub(crate) fn is_empty(&self) -> bool {
        self.looking_vars.is_empty() && self.looked_vars.is_empty()
    }
}

impl<'a, F: RichField + Extendable<D>, const D: usize>
    LogupCheckVars<F, F::Extension, F::Extension, D>
{
    pub(crate) fn from_proofs<C: GenericConfig<D, F = F>>(
        proofs: &[StarkProof<F, C, D>; NUM_TABLES],
        cross_table_logups: &'a [CrossTableLogup],
        ctl_challenges: &'a GrandProductChallengeSet<F>,
    ) -> [Self; NUM_TABLES] {
        let mut looking_column_indices_per_table = [0; NUM_TABLES].map(|_| vec![]);
        let mut looked_column_indices_per_table = [0; NUM_TABLES].map(|_| vec![]);

        for _ in &ctl_challenges.challenges {
            for logup in cross_table_logups {
                for looking_table in &logup.looking_tables {
                    looking_column_indices_per_table[looking_table.kind as usize].push(
                        looking_table
                            .columns
                            .iter()
                            .map(|c| F::from_canonical_usize(*c))
                            .collect::<Vec<_>>(),
                    );
                }

                looked_column_indices_per_table[logup.looked_table.kind as usize]
                    .push(F::from_canonical_usize(logup.looked_table.table_column));
            }
        }

        let challenges = ctl_challenges
            .challenges
            .iter()
            .map(|c| c.beta)
            .collect::<Vec<_>>();

        let mut logup_check_vars_per_table: [LogupCheckVars<F, _, _, D>; NUM_TABLES] =
            [0; NUM_TABLES].map(|_| LogupCheckVars {
                looking_vars: vec![],
                looked_vars: vec![],
            });

        for (i, (p, looking, looked)) in izip!(
            proofs,
            looking_column_indices_per_table,
            looked_column_indices_per_table
        )
        .enumerate()
        {
            println!("from_proofs, looking.len={}", looking.len());
            println!("from_proofs, looked.len={}", looked.len());
            let openings = &p.openings;

            let aux_polys = &openings.aux_polys;
            let aux_polys_next = &openings.aux_polys_next;

            let mut looking_start: usize = 0;
            let mut looking_end: usize = 0;
            for looking_indices in &looking {
                looking_end += looking.len() + 1;
                let lookup_check_vars = LookupCheckVars {
                    local_values: aux_polys[looking_start..looking_end].to_vec(),
                    next_values: aux_polys_next[looking_start..looking_end].to_vec(),
                    columns: looking_indices.clone(),
                    challenges: challenges.clone(),
                };

                logup_check_vars_per_table[i]
                    .looking_vars
                    .push(lookup_check_vars);

                looking_start = looking_end;
            }

            let mut looked_start: usize = looking_end;
            let mut looked_end: usize = looking_end;
            for looked_column in &looked {
                // looked, mult, z_looked
                looked_end += 3;
                let lookup_check_vars = LookupCheckVars {
                    local_values: aux_polys[looked_start..looked_end].to_vec(),
                    next_values: aux_polys_next[looked_start..looked_end].to_vec(),
                    columns: vec![looked_column.clone()],
                    challenges: challenges.clone(),
                };
                logup_check_vars_per_table[i]
                    .looked_vars
                    .push(lookup_check_vars);

                looked_start = looking_end;
            }
        }

        logup_check_vars_per_table
    }
}
