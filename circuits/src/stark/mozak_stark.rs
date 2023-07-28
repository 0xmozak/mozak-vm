use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::rangecheck::stark::RangeCheckStark;
use crate::shift_amount::stark::ShiftAmountStark;
use crate::{bitwise, cpu, rangecheck, shift_amount};

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub bitwise_stark: BitwiseStark<F, D>,
    pub shift_amount_stark: ShiftAmountStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 4],
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            rangecheck_stark: RangeCheckStark::default(),
            bitwise_stark: BitwiseStark::default(),
            shift_amount_stark: ShiftAmountStark::default(),
            cross_table_lookups: [
                RangecheckCpuTable::lookups(),
                BitwiseCpuTable::lookups(),
                InnerShiftAmountTable::lookups(),
                ShiftAmountCpuTable::lookups(),
            ],
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.num_permutation_batches(config),
            self.rangecheck_stark.num_permutation_batches(config),
            self.bitwise_stark.num_permutation_batches(config),
            self.shift_amount_stark.num_permutation_batches(config),
        ]
    }

    pub(crate) fn permutation_batch_sizes(&self) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.permutation_batch_size(),
            self.rangecheck_stark.permutation_batch_size(),
            self.bitwise_stark.permutation_batch_size(),
            self.shift_amount_stark.permutation_batch_size(),
        ]
    }
}

pub(crate) const NUM_TABLES: usize = 4;

#[derive(Debug, Copy, Clone)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
    Bitwise = 2,
    ShiftAmount = 3,
}

impl TableKind {
    #[must_use]
    pub fn all() -> [TableKind; 4] {
        [
            TableKind::Cpu,
            TableKind::RangeCheck,
            TableKind::Bitwise,
            TableKind::ShiftAmount,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct Table<F: Field> {
    pub(crate) kind: TableKind,
    pub(crate) columns: Vec<Column<F>>,
    pub(crate) filter_column: Column<F>,
}

impl<F: Field> Table<F> {
    pub fn new(kind: TableKind, columns: Vec<Column<F>>, filter_column: Column<F>) -> Self {
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

/// Represents a bitwise trace table in the Mozak VM.
pub struct BitwiseTable<F: Field>(Table<F>);

/// Represents a shift amount trace table in the Mozak VM.
pub struct ShiftAmountTable<F: Field>(Table<F>);

impl<F: Field> RangeCheckTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::RangeCheck, columns, filter_column)
    }
}

impl<F: Field> CpuTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::Cpu, columns, filter_column)
    }
}

impl<F: Field> BitwiseTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::Bitwise, columns, filter_column)
    }
}

impl<F: Field> ShiftAmountTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::ShiftAmount, columns, filter_column)
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
                cpu::columns::filter_for_rangecheck(),
            )],
            RangeCheckTable::new(
                rangecheck::columns::data_for_cpu(),
                rangecheck::columns::filter_for_cpu(),
            ),
        )
    }
}

pub struct BitwiseCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for BitwiseCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_bitwise(),
                cpu::columns::filter_for_bitwise(),
            )],
            BitwiseTable::new(
                bitwise::columns::data_for_cpu(),
                bitwise::columns::filter_for_cpu(),
            ),
        )
    }
}

pub struct InnerShiftAmountTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for InnerShiftAmountTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![ShiftAmountTable::new(
                shift_amount::columns::data_for_fixed_value(),
                Column::always(),
            )],
            ShiftAmountTable::new(shift_amount::columns::data_for_inst(), Column::always()),
        )
    }
}

pub struct ShiftAmountCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for ShiftAmountCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_shift_amount(),
                cpu::columns::filter_for_shift_amount(),
            )],
            ShiftAmountTable::new(
                shift_amount::columns::data_for_cpu(),
                shift_amount::columns::filter_for_cpu(),
            ),
        )
    }
}
