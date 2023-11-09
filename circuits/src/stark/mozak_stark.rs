use itertools::{chain, Itertools};
use mozak_circuits_derive::{StarkSet, stark_lambda};
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use serde::{Deserialize, Serialize};
use starky::config::StarkConfig;

use crate::bitshift::stark::BitshiftStark;
use crate::columns_view::columns_view_impl;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::memory::stark::MemoryStark;
use crate::memory_fullword::stark::FullWordMemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memory_io::stark::InputOuputMemoryStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::poseidon2::stark::Poseidon2_12Stark;
use crate::poseidon2_sponge::stark::Poseidon2SpongeStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::columns::rangecheck_looking;
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_limb::stark::RangeCheckLimbStark;
use crate::register::stark::RegisterStark;
use crate::registerinit::stark::RegisterInitStark;
use crate::xor::stark::XorStark;
use crate::{
    bitshift, cpu, memory, memory_fullword, memory_halfword, memory_io, memoryinit,
    poseidon2_sponge, program, rangecheck, xor,
};

#[derive(Clone, StarkSet)]
#[StarkSet(enum_name = "TableKind", field = "F", degree = "D")]
pub struct MozakStark<F: RichField + Extendable<D>, const D: usize> {
    #[StarkSet(stark_kind = "Cpu")]
    pub cpu_stark: CpuStark<F, D>,
    #[StarkSet(stark_kind = "RangeCheck")]
    pub rangecheck_stark: RangeCheckStark<F, D>,
    #[StarkSet(stark_kind = "Xor")]
    pub xor_stark: XorStark<F, D>,
    #[StarkSet(stark_kind = "Bitshift")]
    pub shift_amount_stark: BitshiftStark<F, D>,
    #[StarkSet(stark_kind = "Program")]
    pub program_stark: ProgramStark<F, D>,
    #[StarkSet(stark_kind = "Memory")]
    pub memory_stark: MemoryStark<F, D>,
    #[StarkSet(stark_kind = "MemoryInit")]
    pub memory_init_stark: MemoryInitStark<F, D>,
    #[StarkSet(stark_kind = "RangeCheckLimb")]
    pub rangecheck_limb_stark: RangeCheckLimbStark<F, D>,
    #[StarkSet(stark_kind = "HalfWordMemory")]
    pub halfword_memory_stark: HalfWordMemoryStark<F, D>,
    #[StarkSet(stark_kind = "FullWordMemory")]
    pub fullword_memory_stark: FullWordMemoryStark<F, D>,
    #[StarkSet(stark_kind = "RegisterInit")]
    pub register_init_stark: RegisterInitStark<F, D>,
    #[StarkSet(stark_kind = "Register")]
    pub register_stark: RegisterStark<F, D>,
    #[StarkSet(stark_kind = "IoMemory")]
    pub io_memory_stark: InputOuputMemoryStark<F, D>,
    #[StarkSet(stark_kind = "Poseidon2Sponge")]
    pub poseidon2_sponge_stark: Poseidon2SpongeStark<F, D>,
    #[StarkSet(stark_kind = "Poseidon2")]
    pub poseidon2_stark: Poseidon2_12Stark<F, D>,
    pub cross_table_lookups: [CrossTableLookup<F>; 14],
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
            io_memory_stark: InputOuputMemoryStark::default(),
            poseidon2_sponge_stark: Poseidon2SpongeStark::default(),
            poseidon2_stark: Poseidon2_12Stark::default(),
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
                IoMemoryCpuTable::lookups(),
                Poseidon2SpongeCpuTable::lookups(),
                Poseidon2Poseidon2SpongeTable::lookups(),
            ],
            debug: false,
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; TableKind::COUNT] {
        self.all_starks(stark_lambda!(
            // F and D for `Stark<F, D>`
            F, D,
            // Any generics that need to be propagated
            // e.g. `<T> where T: Copy {},`
            <'a>,
            // Captures
            (config): (&'a StarkConfig),
            // The "lambda"
            |config, stark, _kind: TableKind| -> usize {
                stark.num_permutation_batches(config)
            }
        ))
    }
    pub(crate) fn permutation_batch_sizes(&self) -> [usize; TableKind::COUNT] {
        self.all_starks(stark_lambda!(
            // F and D for `Stark<F, D>`
            F, D,
            // The "lambda"
            |stark, _kind: TableKind| -> usize {
                stark.permutation_batch_size()
            }
        ))
    }
    #[must_use]
    pub fn default_debug() -> Self {
        Self {
            debug: true,
            ..Self::default()
        }
    }
}

// TODO: Remove
pub(crate) const NUM_TABLES: usize = TableKind::COUNT;

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

table_impl!(CpuTable, TableKind::Cpu);
table_impl!(RangeCheckTable, TableKind::RangeCheck);
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
table_impl!(IoMemoryTable, TableKind::IoMemory);
table_impl!(Poseidon2SpongeTable, TableKind::Poseidon2Sponge);
table_impl!(Poseidon2Table, TableKind::Poseidon2);

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
    #[allow(clippy::too_many_lines)]
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
                IoMemoryTable::new(
                    memory_io::columns::data_for_memory(),
                    memory_io::columns::filter_for_memory(),
                ),
                // poseidon2_sponge input
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(0),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(1),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(2),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(3),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(4),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(5),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(6),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                ),
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(7),
                    poseidon2_sponge::columns::filter_for_input_memory(),
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
pub struct IoMemoryCpuTable<F: Field>(CrossTableLookup<F>);

impl<F: Field> Lookups<F> for IoMemoryCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_io_memory(),
                cpu::columns::filter_for_io_memory(),
            )],
            IoMemoryTable::new(
                memory_io::columns::data_for_cpu(),
                memory_io::columns::filter_for_cpu(),
            ),
        )
    }
}

pub struct Poseidon2SpongeCpuTable<F: Field>(CrossTableLookup<F>);
impl<F: Field> Lookups<F> for Poseidon2SpongeCpuTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![Poseidon2SpongeTable::new(
                crate::poseidon2_sponge::columns::data_for_cpu(),
                crate::poseidon2_sponge::columns::filter_for_cpu(),
            )],
            CpuTable::new(
                crate::cpu::columns::data_for_poseidon2_sponge(),
                crate::cpu::columns::filter_for_poseidon2_sponge(),
            ),
        )
    }
}

pub struct Poseidon2Poseidon2SpongeTable<F: Field>(CrossTableLookup<F>);
impl<F: Field> Lookups<F> for Poseidon2Poseidon2SpongeTable<F> {
    fn lookups() -> CrossTableLookup<F> {
        CrossTableLookup::new(
            vec![Poseidon2Table::new(
                crate::poseidon2::columns::data_for_sponge(),
                crate::poseidon2::columns::filter_for_sponge(),
            )],
            Poseidon2SpongeTable::new(
                crate::poseidon2_sponge::columns::data_for_poseidon2(),
                crate::poseidon2_sponge::columns::filter_for_poseidon2(),
            ),
        )
    }
}
