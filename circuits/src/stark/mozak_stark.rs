use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::bitshift::stark::BitshiftStark;
use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::program::stark::ProgramStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::{bitshift, bitwise, cpu, program, rangecheck};

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub bitwise_stark: BitwiseStark<F, D>,
    pub program_stark: ProgramStark<F, D>,
    pub shift_amount_stark: BitshiftStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 5],
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            rangecheck_stark: RangeCheckStark::default(),
            bitwise_stark: BitwiseStark::default(),
            shift_amount_stark: BitshiftStark::default(),
            program_stark: ProgramStark::default(),
            cross_table_lookups: [
                RangecheckCpuTable::lookups(),
                BitwiseCpuTable::lookups(),
                BitshiftCpuTable::lookups(),
                InnerCpuTable::lookups(),
                ProgramCpuTable::lookups(),
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
            self.program_stark.num_permutation_batches(config),
        ]
    }

    pub(crate) fn permutation_batch_sizes(&self) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.permutation_batch_size(),
            self.rangecheck_stark.permutation_batch_size(),
            self.bitwise_stark.permutation_batch_size(),
            self.shift_amount_stark.permutation_batch_size(),
            self.program_stark.permutation_batch_size(),
        ]
    }
}

pub(crate) const NUM_TABLES: usize = 5;

#[derive(Debug, Copy, Clone)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
    Bitwise = 2,
    Bitshift = 3,
    Program = 4,
}

impl TableKind {
    #[must_use]
    pub fn all() -> [TableKind; NUM_TABLES] {
        [
            TableKind::Cpu,
            TableKind::RangeCheck,
            TableKind::Bitwise,
            TableKind::Bitshift,
            TableKind::Program,
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
pub struct BitshiftTable<F: Field>(Table<F>);

/// Represents a program trace table in the Mozak VM.
pub struct ProgramTable<F: Field>(Table<F>);

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

impl<F: Field> BitshiftTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::Bitshift, columns, filter_column)
    }
}

impl<F: Field> ProgramTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::Program, columns, filter_column)
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

pub struct BitshiftCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for BitshiftCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_shift_amount(),
                cpu::columns::filter_for_shift_amount(),
            )],
            BitshiftTable::new(
                bitshift::columns::data_for_cpu(),
                bitshift::columns::filter_for_cpu(),
            ),
        )
    }
}

pub struct InnerCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for InnerCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_inst(),
                Column::always(),
            )],
            CpuTable::new(cpu::columns::data_for_permuted_inst(), Column::always()),
        )
    }
}

pub struct ProgramCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for ProgramCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_permuted_inst(),
                Column::single(cpu::columns::MAP.permuted.filter),
            )],
            ProgramTable::new(
                program::columns::data_for_ctl(),
                Column::single(program::columns::MAP.filter),
            ),
        )
    }
}
