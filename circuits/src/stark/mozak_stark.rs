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
use crate::memory::stark::MemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::columns::rangecheck_looking;
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_limb::stark::RangeCheckLimbStark;
use crate::xor::stark::XorStark;
use crate::{bitshift, cpu, memory, memoryinit, program, rangecheck, xor};

#[derive(Clone)]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    pub cpu_stark: CpuStark<F, D>,
    pub rangecheck_stark: RangeCheckStark<F, D>,
    pub xor_stark: XorStark<F, D>,
    pub shift_amount_stark: BitshiftStark<F, D>,
    pub program_stark: ProgramStark<F, D>,
    pub memory_stark: MemoryStark<F, D>,
    pub memory_init_stark: MemoryInitStark<F, D>,
    pub rangecheck_limb_stark: RangeCheckLimbStark<F, D>,
    pub halfword_memory_stark: HalfWordMemoryStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 8],
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
            rangecheck_limb_stark: RangeCheckLimbStark::default(),
            halfword_memory_stark: HalfWordMemoryStark::default(),
            cross_table_lookups: [
                RangecheckTable::lookups(),
                XorCpuTable::lookups(),
                BitshiftCpuTable::lookups(),
                InnerCpuTable::lookups(),
                ProgramCpuTable::lookups(),
                MemoryCpuTable::lookups(),
                MemoryInitMemoryTable::lookups(),
                LimbTable::lookups(),
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
            self.rangecheck_limb_stark.num_permutation_batches(config),
            self.halfword_memory_stark.num_permutation_batches(config),
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
            self.rangecheck_limb_stark.permutation_batch_size(),
            self.halfword_memory_stark.permutation_batch_size(),
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

pub(crate) const NUM_TABLES: usize = 9;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TableKind {
    Cpu = 0,
    RangeCheck = 1,
    Xor = 2,
    Bitshift = 3,
    Program = 4,
    Memory = 5,
    MemoryInit = 6,
    RangeCheckLimb = 7,
    HalfWordMemory = 8,
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
            TableKind::RangeCheckLimb,
            TableKind::HalfWordMemory,
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

/// Macro to instantiate a new table for cross table lookups.
macro_rules! table_impl {
    ($t: ident, $tk: expr) => {
        pub struct $t<F: Field>(Table<F>);

        impl<F: Field> $t<F> {
            #[allow(clippy::new_ret_no_self)]
            pub fn new(columns: Vec<Column<F>>, filter_column: Column<F>) -> Table<F> {
                Table::new($tk, columns, filter_column)
            }
        }
    };
}

table_impl!(RangeCheckTable, TableKind::RangeCheck);
table_impl!(CpuTable, TableKind::Cpu);
table_impl!(XorTable, TableKind::Xor);
table_impl!(BitshiftTable, TableKind::Bitshift);
table_impl!(ProgramTable, TableKind::Program);
table_impl!(MemoryTable, TableKind::Memory);
table_impl!(MemoryInitTable, TableKind::MemoryInit);
table_impl!(RangeCheckLimbTable, TableKind::RangeCheckLimb);

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

pub struct MemoryInitMemoryTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for MemoryInitMemoryTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![MemoryTable::new(
                memory::columns::data_for_memoryinit(),
                memory::columns::filter_for_memoryinit(),
            )],
            MemoryInitTable::new(
                memoryinit::columns::data_for_memory(),
                memoryinit::columns::filter_for_memory(),
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

pub struct LimbTable<F: Field>(CrossTableLookup<F>);
impl<F: Field> Lookups<F> for LimbTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            rangecheck_looking(),
            RangeCheckLimbTable::new(
                crate::rangecheck_limb::columns::data(),
                crate::rangecheck_limb::columns::filter(),
            ),
        )
    }
}
