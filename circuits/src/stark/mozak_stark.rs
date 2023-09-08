use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use serde::{Deserialize, Serialize};
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::bitshift::stark::BitshiftStark;
use crate::columns_view::columns_view_impl;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::generation::memoryinit;
use crate::memory::stark::MemoryStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::xor::stark::XorStark;
use crate::{bitshift, cpu, memory, program, rangecheck, xor};

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub xor_stark: XorStark<F, D>,
    pub shift_amount_stark: BitshiftStark<F, D>,
    pub program_stark: ProgramStark<F, D>,
    pub memory_stark: MemoryStark<F, D>,
    pub memory_init_stark: MemoryInitStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 6],
    pub debug: bool,
}

columns_view_impl!(PublicInputs);

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(bound = "F: Field")]
pub struct PublicInputs<F> {
    pub entry_point: F,
}

impl<F: RichField + Extendable<D>, const D: usize> Default for MozakStark<F, D> {
    fn default() -> Self {
        Self {
            cpu_stark: CpuStark::default(),
            rangecheck_stark: RangeCheckStark::default(),
            xor_stark: XorStark::default(),
            shift_amount_stark: BitshiftStark::default(),
            program_stark: ProgramStark::default(),
            memory_stark: MemoryStark::default(),
            memory_init_stark: MemoryInitStark::default(),
            cross_table_lookups: [
                RangecheckTable::lookups(),
                XorCpuTable::lookups(),
                BitshiftCpuTable::lookups(),
                InnerCpuTable::lookups(),
                ProgramCpuTable::lookups(),
                MemoryCpuTable::lookups(),
            ],
            debug: false,
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.num_permutation_batches(config),
            self.rangecheck_stark.num_permutation_batches(config),
            self.xor_stark.num_permutation_batches(config),
            self.shift_amount_stark.num_permutation_batches(config),
            self.program_stark.num_permutation_batches(config),
            self.memory_stark.num_permutation_batches(config),
            self.memory_init_stark.num_permutation_batches(config),
        ]
    }

    pub(crate) fn permutation_batch_sizes(&self) -> [usize; NUM_TABLES] {
        [
            self.cpu_stark.permutation_batch_size(),
            self.rangecheck_stark.permutation_batch_size(),
            self.xor_stark.permutation_batch_size(),
            self.shift_amount_stark.permutation_batch_size(),
            self.program_stark.permutation_batch_size(),
            self.memory_stark.permutation_batch_size(),
            self.memory_init_stark.permutation_batch_size(),
        ]
    }

    #[must_use]
    pub fn default_debug() -> Self {
        Self {
            debug: true,
            ..Self::default()
        }
    }
}

pub(crate) const NUM_TABLES: usize = 7;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
    Xor = 2,
    Bitshift = 3,
    Program = 4,
    Memory = 5,
    MemoryInit = 6,
}

impl TableKind {
    #[must_use]
    pub fn all() -> [TableKind; NUM_TABLES] {
        [
            TableKind::Cpu,
            TableKind::RangeCheck,
            TableKind::Xor,
            TableKind::Bitshift,
            TableKind::Program,
            TableKind::Memory,
            TableKind::MemoryInit,
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

/// Represents a xor trace table in the Mozak VM.
pub struct XorTable<F: Field>(Table<F>);

/// Represents a shift amount trace table in the Mozak VM.
pub struct BitshiftTable<F: Field>(Table<F>);

/// Represents a program trace table in the Mozak VM.
pub struct ProgramTable<F: Field>(Table<F>);

/// Represents a memory trace table in the Mozak VM.
pub struct MemoryTable<F: Field>(Table<F>);

/// Represents a memory init table in the Mozak VM.
pub struct MemoryInitTable<F: Field>(Table<F>);

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

impl<F: Field> XorTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::Xor, columns, filter_column)
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

impl<F: Field> MemoryTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::Memory, columns, filter_column)
    }
}

impl<F: Field> MemoryInitTable<F> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
        Table::new(TableKind::MemoryInit, columns, filter_column)
    }
}

pub trait Lookups<F: Field> {
    fn lookups() -> CrossTableLookup<F>;
}

pub struct RangecheckTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for RangecheckTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        let looking: Vec<Table<F>> = chain![
            memory::columns::rangecheck_looking(),
            cpu::columns::rangecheck_looking(),
        ]
        .collect();
        CrossTableLookup::new(
            looking,
            RangeCheckTable::new(rangecheck::columns::data(), rangecheck::columns::filter()),
        )
    }
}

pub struct XorCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for XorCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_xor(),
                cpu::columns::filter_for_xor(),
            )],
            XorTable::new(xor::columns::data_for_cpu(), xor::columns::filter_for_cpu()),
        )
    }
}

pub struct MemoryCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for MemoryCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_memory(),
                cpu::columns::filter_for_memory(),
            )],
            MemoryTable::new(
                memory::columns::data_for_cpu(),
                memory::columns::filter_for_cpu(),
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
                Column::single(cpu::columns::MAP.cpu.is_running),
            )],
            CpuTable::new(
                cpu::columns::data_for_permuted_inst(),
                Column::single(cpu::columns::MAP.cpu.is_running),
            ),
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

pub struct MemoryInitMemoryTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for MemoryInitMemoryTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![MemoryInitTable::new(
                memoryinit::columns::data_for_memory(),
                memoryinit::columns::filter_for_memory(),
            )],
            MemoryTable::new(
                memory::columns::data_for_memoryinit(),
                memory::columns::filter_for_memoryinit(),
            ),
        )
    }
}
