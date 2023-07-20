use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::rangecheck::stark::RangeCheckStark;
use crate::{cpu, rangecheck};

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 2],
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            rangecheck_stark: RangeCheckStark::default(),
            cross_table_lookups: [
                CpuDstValueRangeCheckTable::lookups(),
                CpuOp1ValueFixedRangeCheckTable::lookups(),
            ],
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.num_permutation_batches(config),
            self.rangecheck_stark.num_permutation_batches(config),
        ]
    }

    pub(crate) fn permutation_batch_sizes(&self) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.permutation_batch_size(),
            self.rangecheck_stark.permutation_batch_size(),
        ]
    }
}

pub(crate) const NUM_TABLES: usize = 2;

#[derive(Debug, Copy, Clone)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
}

impl TableKind {
    #[must_use]
    pub fn all() -> [TableKind; 2] { [TableKind::Cpu, TableKind::RangeCheck] }
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

pub struct CpuDstValueRangeCheckTable<F: Field>(CrossTableLookup<F>);
pub struct CpuOp1ValueFixedRangeCheckTable<F: Field>(CrossTableLookup<F>);
pub struct CpuOp2ValueFixedRangeCheckTable<F: Field>(CrossTableLookup<F>);
pub struct CpuCmpAbsDiffRangeCheckTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for CpuDstValueRangeCheckTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_dst_value_rangecheck(),
                Some(cpu::columns::filter_for_add_rangecheck()),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                Some(rangecheck::columns::filter_cpu_dst_value()),
            ),
        )
    }
}

impl<F: Field> Lookups<F> for CpuOp1ValueFixedRangeCheckTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_op1_val_fixed_rangecheck(),
                Some(cpu::columns::filter_for_slt_rangecheck()),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                Some(rangecheck::columns::filter_cpu_op1_val_fixed()),
            ),
        )
    }
}
impl<F: Field> Lookups<F> for CpuOp2ValueFixedRangeCheckTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_op2_val_fixed_rangecheck(),
                Some(cpu::columns::filter_for_slt_rangecheck()),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                Some(rangecheck::columns::filter_cpu_op2_val_fixed()),
            ),
        )
    }
}
impl<F: Field> Lookups<F> for CpuCmpAbsDiffRangeCheckTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_cmp_abs_diff_rangecheck(),
                Some(cpu::columns::filter_for_slt_rangecheck()),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                Some(rangecheck::columns::filter_cpu_cmp_abs_diff()),
            ),
        )
    }
}
