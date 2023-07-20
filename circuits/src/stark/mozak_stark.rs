use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::rangecheck::stark::RangeCheckStark;
use crate::{cpu, rangecheck};

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub bitwise_stark: BitwiseStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 1],
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            rangecheck_stark: RangeCheckStark::default(),
            bitwise_stark: BitwiseStark::default(),
            cross_table_lookups: [RangecheckCpuTable::lookups(); 1],
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.num_permutation_batches(config),
            self.rangecheck_stark.num_permutation_batches(config),
            self.bitwise_stark.num_permutation_batches(config),
        ]
    }

    pub(crate) fn permutation_batch_sizes(&self) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.permutation_batch_size(),
            self.rangecheck_stark.permutation_batch_size(),
            self.bitwise_stark.permutation_batch_size(),
        ]
    }
}

pub(crate) const NUM_TABLES: usize = 3;

#[derive(Debug, Copy, Clone)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
    Bitwise = 2,
}

impl TableKind {
    #[must_use]
    pub fn all() -> [TableKind; 3] { [TableKind::Cpu, TableKind::RangeCheck, TableKind::Bitwise] }
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Table<F: Field> {
    pub(crate) kind: TableKind,
    pub(crate) columns: Vec<Column<F>>,
    pub(crate) filter_column: Option<Column<F>>,
}

impl<F: Field> Table<F> {
    pub fn new(kind: TableKind, columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Self {
        Self {
            kind,
            columns,
            filter_column,
        }
    }
}

/// Represents a range check trace table in the Mozak VM.
pub struct RangeCheckTable<F: Field>(Table<F>);

/// Represents a cpu trace table in the Mozak VM.
pub struct CpuTable<F: Field>(Table<F>);

impl<F: Field> RangeCheckTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Table<F> {
        Table::new(TableKind::RangeCheck, columns, filter_column)
    }
}

impl<F: Field> CpuTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Option<Column<F>>) -> Table<F> {
        Table::new(TableKind::Cpu, columns, filter_column)
    }
}
pub trait Lookups<F: Field> {
    fn lookups() -> CrossTableLookup<F>;
}

pub struct RangecheckCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for RangecheckCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_rangecheck(),
                Some(cpu::columns::filter_for_rangecheck()),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                Some(rangecheck::columns::filter_for_cpu()),
            ),
        )
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::test_utils::simple_test;
    use plonky2::fri::FriConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::log2_ceil;
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::stark::Stark;

    use crate::stark::mozak_stark::MozakStark;
    use crate::stark::prover::prove;
    use crate::stark::verifier::verify_proof;

    #[test]
    fn mozak_e2e_test() {
        let inst = 0x0073_02b3 /* add r5, r6, r7 */;
        let mut mem = vec![];
        let u16max: u32 = u32::from(u16::MAX);
        for i in 0..u16max {
            mem.push((i * 4, inst));
        }
        let record = simple_test(4 * u16max, &mem, &[(6, 100), (7, 100)]);
        let step_rows = &record.executed;

        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = MozakStark<F, D>;
        let mut stark = S::default();
        let config = StarkConfig::standard_fast_config();
        let config = StarkConfig {
            security_bits: 1,
            num_challenges: 2,
            fri_config: FriConfig {
                // Plonky2 says: "Having constraints of degree higher than the rate is not supported
                // yet." So we automatically set the rate here as required by plonky2.
                rate_bits: log2_ceil(stark.cpu_stark.constraint_degree()),
                cap_height: 0,
                proof_of_work_bits: 0,
                num_query_rounds: 5,
                ..config.fri_config
            },
        };

        let all_proof =
            prove::<F, C, D>(step_rows, &mut stark, &config, &mut TimingTree::default());
        verify_proof(stark, all_proof.unwrap(), &config).unwrap();
    }
}
