use itertools::{chain, Itertools};
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
use crate::memory_fullword::stark::FullWordMemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memory_io::stark::InputOuputMemoryStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::columns::rangecheck_looking;
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_limb::stark::RangeCheckLimbStark;
use crate::register::stark::RegisterStark;
use crate::registerinit::stark::RegisterInitStark;
use crate::xor::stark::XorStark;
use crate::{
    bitshift, cpu, memory, memory_fullword, memory_halfword, memory_io, memoryinit, program,
    rangecheck, xor,
};

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
    pub fullword_memory_stark: FullWordMemoryStark<F, D>,
    pub io_memory_private_stark: InputOuputMemoryStark<F, D>,
    pub io_memory_public_stark: InputOuputMemoryStark<F, D>,
    pub register_init_stark: RegisterInitStark<F, D>,
    pub register_stark: RegisterStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 13],
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
            fullword_memory_stark: FullWordMemoryStark::default(),
            register_init_stark: RegisterInitStark::default(),
            register_stark: RegisterStark::default(),
            io_memory_private_stark: InputOuputMemoryStark::default(),
            io_memory_public_stark: InputOuputMemoryStark::default(),
            cross_table_lookups: [
                RangecheckTable::lookups(),
                XorCpuTable::lookups(),
                BitshiftCpuTable::lookups(),
                InnerCpuTable::lookups(),
                ProgramCpuTable::lookups(),
                IntoMemoryTable::lookups(),
                MemoryInitMemoryTable::lookups(),
                LimbTable::lookups(),
                HalfWordMemoryCpuTable::lookups(),
                FullWordMemoryCpuTable::lookups(),
                RegisterRegInitTable::lookups(),
                IoMemoryPrivateCpuTable::lookups(),
                IoMemoryPublicCpuTable::lookups(),
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
            self.fullword_memory_stark.num_permutation_batches(config),
            self.register_init_stark.num_permutation_batches(config),
            self.register_stark.num_permutation_batches(config),
            self.io_memory_private_stark.num_permutation_batches(config),
            self.io_memory_public_stark.num_permutation_batches(config),
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
            self.fullword_memory_stark.permutation_batch_size(),
            self.register_init_stark.permutation_batch_size(),
            self.register_stark.permutation_batch_size(),
            self.io_memory_private_stark.permutation_batch_size(),
            self.io_memory_public_stark.permutation_batch_size(),
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

pub(crate) const NUM_TABLES: usize = 14;

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
    FullWordMemory = 9,
    RegisterInit = 10,
    Register = 11,
    IoMemoryPrivate = 12,
    IoMemoryPublic = 13,
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
            TableKind::FullWordMemory,
            TableKind::RegisterInit,
            TableKind::Register,
            TableKind::IoMemoryPrivate,
            TableKind::IoMemoryPublic,
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
table_impl!(HalfWordMemoryTable, TableKind::HalfWordMemory);
table_impl!(FullWordMemoryTable, TableKind::FullWordMemory);
table_impl!(RegisterInitTable, TableKind::RegisterInit);
table_impl!(RegisterTable, TableKind::Register);
table_impl!(IoMemoryPrivateTable, TableKind::IoMemoryPrivate);
table_impl!(IoMemoryPublicTable, TableKind::IoMemoryPublic);

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

pub struct IntoMemoryTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for IntoMemoryTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![
                CpuTable::new(
                    cpu::columns::data_for_memory(),
                    cpu::columns::filter_for_byte_memory(),
                ),
                HalfWordMemoryTable::new(
                    memory_halfword::columns::data_for_memory_limb(0),
                    memory_halfword::columns::filter(),
                ),
                HalfWordMemoryTable::new(
                    memory_halfword::columns::data_for_memory_limb(1),
                    memory_halfword::columns::filter(),
                ),
                FullWordMemoryTable::new(
                    memory_fullword::columns::data_for_memory_limb(0),
                    memory_fullword::columns::filter(),
                ),
                FullWordMemoryTable::new(
                    memory_fullword::columns::data_for_memory_limb(1),
                    memory_fullword::columns::filter(),
                ),
                FullWordMemoryTable::new(
                    memory_fullword::columns::data_for_memory_limb(2),
                    memory_fullword::columns::filter(),
                ),
                FullWordMemoryTable::new(
                    memory_fullword::columns::data_for_memory_limb(3),
                    memory_fullword::columns::filter(),
                ),
                IoMemoryPrivateTable::new(
                    memory_io::columns::data_for_memory(),
                    memory_io::columns::filter_for_memory(),
                ),
                IoMemoryPublicTable::new(
                    memory_io::columns::data_for_memory(),
                    memory_io::columns::filter_for_memory(),
                ),
            ],
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
            chain!(rangecheck_looking(), cpu::columns::rangecheck_looking_u8(),).collect_vec(),
            RangeCheckLimbTable::new(
                crate::rangecheck_limb::columns::data(),
                crate::rangecheck_limb::columns::filter(),
            ),
        )
    }
}

pub struct HalfWordMemoryCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for HalfWordMemoryCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_halfword_memory(),
                cpu::columns::filter_for_halfword_memory(),
            )],
            HalfWordMemoryTable::new(
                memory_halfword::columns::data_for_cpu(),
                memory_halfword::columns::filter(),
            ),
        )
    }
}

pub struct FullWordMemoryCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for FullWordMemoryCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_fullword_memory(),
                cpu::columns::filter_for_fullword_memory(),
            )],
            FullWordMemoryTable::new(
                memory_fullword::columns::data_for_cpu(),
                memory_fullword::columns::filter(),
            ),
        )
    }
}

pub struct RegisterRegInitTable<F: Field>(CrossTableLookup<F>);
impl<F: Field> Lookups<F> for RegisterRegInitTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![RegisterTable::new(
                crate::register::columns::data_for_register_init(),
                crate::register::columns::filter_for_register_init(),
            )],
            RegisterInitTable::new(
                crate::registerinit::columns::data_for_register(),
                crate::registerinit::columns::filter_for_register(),
            ),
        )
    }
}
pub struct IoMemoryPrivateCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for IoMemoryPrivateCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_io_memory_private(),
                cpu::columns::filter_for_io_memory_private(),
            )],
            IoMemoryPrivateTable::new(
                memory_io::columns::data_for_cpu(),
                memory_io::columns::filter_for_cpu(),
            ),
        )
    }
}

pub struct IoMemoryPublicCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for IoMemoryPublicCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_io_memory_public(),
                cpu::columns::filter_for_io_memory_public(),
            )],
            IoMemoryPublicTable::new(
                memory_io::columns::data_for_cpu(),
                memory_io::columns::filter_for_cpu(),
            ),
        )
    }
}
