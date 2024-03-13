use std::ops::{Index, IndexMut};

use itertools::chain;
use mozak_circuits_derive::StarkSet;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use serde::{Deserialize, Serialize};

use crate::bitshift::stark::BitshiftStark;
use crate::columns_view::columns_view_impl;
use crate::cpu::stark::CpuStark;
use crate::cross_table_lookup::{Column, CrossTableLookup};
use crate::memory::stark::MemoryStark;
use crate::memory_fullword::stark::FullWordMemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memory_io::stark::InputOutputMemoryStark;
use crate::memory_zeroinit::stark::MemoryZeroInitStark;
use crate::memoryinit::stark::MemoryInitStark;
use crate::poseidon2::stark::Poseidon2_12Stark;
#[cfg(feature = "enable_poseidon_starks")]
use crate::poseidon2_output_bytes;
use crate::poseidon2_output_bytes::stark::Poseidon2OutputBytesStark;
#[cfg(feature = "enable_poseidon_starks")]
use crate::poseidon2_sponge;
use crate::poseidon2_sponge::stark::Poseidon2SpongeStark;
use crate::program::stark::ProgramStark;
use crate::rangecheck::columns::rangecheck_looking;
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_u8::stark::RangeCheckU8Stark;
#[cfg(feature = "enable_register_starks")]
use crate::register;
use crate::register::stark::RegisterStark;
use crate::registerinit::stark::RegisterInitStark;
use crate::xor::stark::XorStark;
use crate::{
    bitshift, cpu, memory, memory_fullword, memory_halfword, memory_io, memory_zeroinit,
    memoryinit, program, rangecheck, xor,
};

const NUM_CROSS_TABLE_LOOKUP: usize = {
    13 + cfg!(feature = "enable_register_starks") as usize * 2
        + cfg!(feature = "enable_poseidon_starks") as usize * 3
};

/// STARK Gadgets of Mozak-VM
///
/// ## Generics
/// `F`: The [Field] that the STARK is defined over
/// `D`: Degree of the extension field of `F`
#[derive(Clone, StarkSet)]
#[StarkSet(macro_name = "mozak_stark_set")]
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
    #[StarkSet(stark_kind = "ElfMemoryInit")]
    pub elf_memory_init_stark: MemoryInitStark<F, D>,
    #[StarkSet(stark_kind = "MozakMemoryInit")]
    pub mozak_memory_init_stark: MemoryInitStark<F, D>,
    // TODO(Bing): find a way to natively constrain zero initializations within
    // the `MemoryStark`, instead of relying on a CTL between this and the
    // `MemoryStark`.
    #[StarkSet(stark_kind = "MemoryZeroInit")]
    pub memory_zeroinit_stark: MemoryZeroInitStark<F, D>,
    #[StarkSet(stark_kind = "RangeCheckU8")]
    pub rangecheck_u8_stark: RangeCheckU8Stark<F, D>,
    #[StarkSet(stark_kind = "HalfWordMemory")]
    pub halfword_memory_stark: HalfWordMemoryStark<F, D>,
    #[StarkSet(stark_kind = "FullWordMemory")]
    pub fullword_memory_stark: FullWordMemoryStark<F, D>,
    #[StarkSet(stark_kind = "IoMemoryPrivate")]
    pub io_memory_private_stark: InputOutputMemoryStark<F, D>,
    #[StarkSet(stark_kind = "IoMemoryPublic")]
    pub io_memory_public_stark: InputOutputMemoryStark<F, D>,
    #[StarkSet(stark_kind = "IoTranscript")]
    pub io_transcript_stark: InputOutputMemoryStark<F, D>,
    #[cfg_attr(
        feature = "enable_register_starks",
        StarkSet(stark_kind = "RegisterInit")
    )]
    pub register_init_stark: RegisterInitStark<F, D>,
    #[cfg_attr(feature = "enable_register_starks", StarkSet(stark_kind = "Register"))]
    pub register_stark: RegisterStark<F, D>,
    #[cfg_attr(feature = "enable_poseidon_starks", StarkSet(stark_kind = "Poseidon2"))]
    pub poseidon2_stark: Poseidon2_12Stark<F, D>,
    #[cfg_attr(
        feature = "enable_poseidon_starks",
        StarkSet(stark_kind = "Poseidon2Sponge")
    )]
    pub poseidon2_sponge_stark: Poseidon2SpongeStark<F, D>,
    #[cfg_attr(
        feature = "enable_poseidon_starks",
        StarkSet(stark_kind = "Poseidon2OutputBytes")
    )]
    pub poseidon2_output_bytes_stark: Poseidon2OutputBytesStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup; NUM_CROSS_TABLE_LOOKUP],

    pub debug: bool,
}

// A macro which takes metadata about `MozakStark`
// and defines
macro_rules! mozak_stark_helpers {
    {
        kind_names = [{ $($kind_names:ident)* }]
        kind_vals = [{ $($kind_vals:literal)* }]
        count = [{ $kind_count:literal }]
        tys = [{ $($tys:ty)* }]
        fields = [{ $($fields:ident)* }]
    } => {
        // Generate all the `TableKind`s and their associated values
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub enum TableKind {
            $($kind_names = $kind_vals,)*
        }

        impl TableKind {
            const COUNT: usize = $kind_count;
        }

        // Generate the set builder
        #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct TableKindSetBuilder<T> {
            $(pub $fields: T,)*
        }

        impl<T> TableKindSetBuilder<T> {
            pub fn from_array(array: TableKindArray<T>) -> Self {
                let TableKindArray([$($fields,)*]) = array;
                Self{$($fields,)*}
            }
            pub fn build(self) -> TableKindArray<T> {
                TableKindArray([$(self.$fields,)*])
            }
            pub fn build_with_kind(self) -> TableKindArray<(T, TableKind)> {
                use TableKind::*;
                TableKindArray([$((self.$fields, $kind_names),)*])
            }
        }

        /// A helper trait needed by `all_kind` in certain situations
        pub trait StarkKinds {
            $(type $kind_names;)*
        }
        impl<F: RichField + Extendable<D>, const D: usize> StarkKinds for MozakStark<F, D> {
            $(type $kind_names = $tys;)*
        }

        // Generate the helper macros

        /// Creates an array by repeatedly calls a "labmda" once per stark type.
        ///
        /// Note that these are not actual lambdas and so early returns will return from
        /// the caller, not the lambda
        ///
        /// Can be called in two ways:
        ///
        /// # With Type
        ///
        /// Calls that need explicit type information of each stark type can provide the parent
        /// `MozakStark` type to the macro in order to enable the "lambdas" to use the `stark!`
        /// macro in-place of a type.
        ///
        /// ```ignore
        /// let foos = all_kind!(MozakStark<F, D>, |stark, kind| {
        ///     // `stark` will be a different stark type on each call
        ///     // `kind` will be a different `TableKind` on each call
        ///     foo::<stark!()>(kind)
        /// });
        /// ```
        ///
        /// # Without Type
        ///
        /// Calls that do not need type information of each stark can merely omit the `MozakStark`
        /// type and just deal with the `TableKind`
        ///
        /// ```ignore
        /// let bars = all_kind!(|stark, kind| bar(kind));
        /// ```
        macro_rules! all_kind {
            ($stark_ty:ty, |$stark:ident, $kind:ident| $val:expr) => {{
                use $crate::stark::mozak_stark::{StarkKinds, TableKindArray, TableKind::*};
                TableKindArray([$(
                    {
                        // This enables callers to get the type using `$stark!()`
                        macro_rules! $stark {
                            () => {<$stark_ty as StarkKinds>::$kind_names}
                        }
                        #[allow(non_upper_case_globals)]
                        const $kind: TableKind = $kind_names;
                        $val
                    },)*
                ])
            }};
            (|$kind:ident| $val:expr) => {{
                use $crate::stark::mozak_stark::{TableKindArray, TableKind::{self, *}};
                TableKindArray([$(
                    {
                        #[allow(non_upper_case_globals)]
                        const $kind: TableKind = $kind_names;
                        $val
                    },)*
                ])
            }};
        }
        pub(crate) use all_kind;

        /// Creates an array by repeated calls to a "lambda" once per stark value.
        ///
        /// Note that these are not actual lambdas and so early returns will return from
        /// the caller, not the lambda
        ///
        /// Calls that need explicit type information of each stark type can provide the parent
        /// `MozakStark` type to the macro in order to enable the "lambdas" to use the `stark!`
        /// macro in-place of a type.
        ///
        /// ```ignore
        /// fn foo(mozak_stark: &mut MozakStark<F, D>) {
        ///     let bars = all_starks!(mozak_stark, |stark, kind| {
        ///         // `stark` will be a reference to different stark on each call
        ///         // `kind` will be a different `TableKind` on each call
        ///         bar(stark, kind)
        ///     });
        ///     let bazs = all_starks!(mozak_stark, |mut stark, kind| {
        ///         // `stark` will be a mutable reference to different stark on each call
        ///         baz(stark, kind)
        ///     });
        /// }
        /// ```
        macro_rules! all_starks {
            ($all_stark:expr, |$stark:ident, $kind:ident| $val:expr) => {{
                use core::borrow::Borrow;
                use $crate::stark::mozak_stark::{TableKindArray, TableKind::*};
                let all_stark = $all_stark.borrow();
                TableKindArray([$(
                    {
                        let $stark = &all_stark.$fields;
                        let $kind = $kind_names;
                        $val
                    },)*
                ])
            }};
            ($all_stark:expr, |mut $stark:ident, $kind:ident| $val:expr) => {{
                use core::borrow::BorrowMut;
                use $crate::stark::mozak_stark::{TableKindArray, TableKind::*};
                let all_stark = $all_stark.borrow_mut();
                TableKindArray([$(
                    {
                        let $stark = &mut all_stark.$fields;
                        let $kind = $kind_names;
                        $val
                    },)*
                ])
            }};
        }
        pub(crate) use all_starks;

    };
}

// Invoke `mozak_stark_set` and pass the result to `mozak_stark_helpers`
// Generating all the helpers we need
tt_call::tt_call! {
    macro = [{ mozak_stark_set }]
    ~~> mozak_stark_helpers
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
pub struct TableKindArray<T>(pub [T; TableKind::COUNT]);

impl<T> Index<TableKind> for TableKindArray<T> {
    type Output = T;

    fn index(&self, kind: TableKind) -> &Self::Output { &self.0[kind as usize] }
}

impl<T> IndexMut<TableKind> for TableKindArray<T> {
    fn index_mut(&mut self, kind: TableKind) -> &mut Self::Output { &mut self.0[kind as usize] }
}

impl<'a, T> IntoIterator for &'a TableKindArray<T> {
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter { self.0.iter() }
}

impl<T> TableKindArray<T> {
    pub fn map<F, U>(self, f: F) -> TableKindArray<U>
    where
        F: FnMut(T) -> U, {
        TableKindArray(self.0.map(f))
    }

    pub fn with_kind(self) -> TableKindArray<(T, TableKind)> {
        TableKindSetBuilder::from_array(self).build_with_kind()
    }

    pub fn each_ref(&self) -> TableKindArray<&T> {
        // TODO: replace with `self.0.each_ref()` (blocked on rust-lang/rust#76118)
        all_kind!(|kind| &self[kind])
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> { self.0.iter() }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> { self.0.iter_mut() }
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
            elf_memory_init_stark: MemoryInitStark::default(),
            mozak_memory_init_stark: MemoryInitStark::default(),
            memory_zeroinit_stark: MemoryZeroInitStark::default(),
            rangecheck_u8_stark: RangeCheckU8Stark::default(),
            halfword_memory_stark: HalfWordMemoryStark::default(),
            fullword_memory_stark: FullWordMemoryStark::default(),
            register_init_stark: RegisterInitStark::default(),
            register_stark: RegisterStark::default(),
            io_memory_private_stark: InputOutputMemoryStark::default(),
            io_memory_public_stark: InputOutputMemoryStark::default(),
            io_transcript_stark: InputOutputMemoryStark::default(),
            poseidon2_sponge_stark: Poseidon2SpongeStark::default(),
            poseidon2_stark: Poseidon2_12Stark::default(),
            poseidon2_output_bytes_stark: Poseidon2OutputBytesStark::default(),

            // These tables contain only descriptions of the tables.
            // The values of the tables are generated as traces.
            cross_table_lookups: [
                RangecheckTable::lookups(),
                XorCpuTable::lookups(),
                BitshiftCpuTable::lookups(),
                InnerCpuTable::lookups(),
                ProgramCpuTable::lookups(),
                IntoMemoryTable::lookups(),
                MemoryInitMemoryTable::lookups(),
                RangeCheckU8LookupTable::lookups(),
                HalfWordMemoryCpuTable::lookups(),
                FullWordMemoryCpuTable::lookups(),
                #[cfg(feature = "enable_register_starks")]
                RegisterRegInitTable::lookups(),
                #[cfg(feature = "enable_register_starks")]
                RegisterLookups::lookups(),
                IoMemoryPrivateCpuTable::lookups(),
                IoMemoryPublicCpuTable::lookups(),
                IoTranscriptCpuTable::lookups(),
                #[cfg(feature = "enable_poseidon_starks")]
                Poseidon2SpongeCpuTable::lookups(),
                #[cfg(feature = "enable_poseidon_starks")]
                Poseidon2Poseidon2SpongeTable::lookups(),
                #[cfg(feature = "enable_poseidon_starks")]
                Poseidon2OutputBytesPoseidon2SpongeTable::lookups(),
            ],
            debug: false,
        }
    }
}

impl<F: RichField + Extendable<D>, const D: usize> MozakStark<F, D> {
    #[must_use]
    pub fn default_debug() -> Self {
        Self {
            debug: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    pub(crate) kind: TableKind,
    pub(crate) columns: Vec<Column>,
    pub(crate) filter_column: Column,
}

impl Table {
    #[must_use]
    pub fn new(kind: TableKind, columns: Vec<Column>, filter_column: Column) -> Self {
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
        pub struct $t;

        impl $t {
            #[allow(clippy::new_ret_no_self)]
            #[must_use]
            pub fn new(columns: Vec<Column>, filter_column: Column) -> Table {
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
table_impl!(ElfMemoryInitTable, TableKind::ElfMemoryInit);
table_impl!(MozakMemoryInitTable, TableKind::MozakMemoryInit);
table_impl!(MemoryZeroInitTable, TableKind::MemoryZeroInit);
table_impl!(RangeCheckU8Table, TableKind::RangeCheckU8);
table_impl!(HalfWordMemoryTable, TableKind::HalfWordMemory);
table_impl!(FullWordMemoryTable, TableKind::FullWordMemory);
#[cfg(feature = "enable_register_starks")]
table_impl!(RegisterInitTable, TableKind::RegisterInit);
#[cfg(feature = "enable_register_starks")]
table_impl!(RegisterTable, TableKind::Register);
table_impl!(IoMemoryPrivateTable, TableKind::IoMemoryPrivate);
table_impl!(IoMemoryPublicTable, TableKind::IoMemoryPublic);
table_impl!(IoTranscriptTable, TableKind::IoTranscript);
#[cfg(feature = "enable_poseidon_starks")]
table_impl!(Poseidon2SpongeTable, TableKind::Poseidon2Sponge);
#[cfg(feature = "enable_poseidon_starks")]
table_impl!(Poseidon2Table, TableKind::Poseidon2);
#[cfg(feature = "enable_poseidon_starks")]
table_impl!(Poseidon2OutputBytesTable, TableKind::Poseidon2OutputBytes);

pub trait Lookups {
    fn lookups() -> CrossTableLookup;
}

pub struct RangecheckTable;

impl Lookups for RangecheckTable {
    fn lookups() -> CrossTableLookup {
        #[cfg(feature = "enable_register_starks")]
        let register = register::columns::rangecheck_looking();
        #[cfg(not(feature = "enable_register_starks"))]
        let register: Vec<Table> = vec![];

        let looking: Vec<Table> = chain![
            memory::columns::rangecheck_looking(),
            cpu::columns::rangecheck_looking(),
            register,
        ]
        .collect();
        CrossTableLookup::new(
            looking,
            RangeCheckTable::new(rangecheck::columns::data(), rangecheck::columns::filter()),
        )
    }
}

pub struct XorCpuTable;

impl Lookups for XorCpuTable {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_xor(),
                cpu::columns::filter_for_xor(),
            )],
            XorTable::new(xor::columns::data_for_cpu(), xor::columns::filter_for_cpu()),
        )
    }
}

pub struct IntoMemoryTable;

impl Lookups for IntoMemoryTable {
    #[allow(clippy::too_many_lines)]
    fn lookups() -> CrossTableLookup {
        let mut tables = vec![];
        tables.extend([
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
        ]);
        #[cfg(feature = "enable_poseidon_starks")]
        {
            tables.extend((0..8).map(|index| {
                Poseidon2SpongeTable::new(
                    poseidon2_sponge::columns::data_for_input_memory(index),
                    poseidon2_sponge::columns::filter_for_input_memory(),
                )
            }));
            tables.extend((0..32).map(|index| {
                Poseidon2OutputBytesTable::new(
                    poseidon2_output_bytes::columns::data_for_output_memory(index),
                    poseidon2_output_bytes::columns::filter_for_output_memory(),
                )
            }));
        }
        CrossTableLookup::new(
            tables,
            MemoryTable::new(
                memory::columns::data_for_cpu(),
                memory::columns::filter_for_cpu(),
            ),
        )
    }
}

pub struct MemoryInitMemoryTable;

impl Lookups for MemoryInitMemoryTable {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            vec![
                ElfMemoryInitTable::new(
                    memoryinit::columns::data_for_memory(),
                    memoryinit::columns::filter_for_memory(),
                ),
                MozakMemoryInitTable::new(
                    memoryinit::columns::data_for_memory(),
                    memoryinit::columns::filter_for_memory(),
                ),
                MemoryZeroInitTable::new(
                    memory_zeroinit::columns::data_for_memory(),
                    memory_zeroinit::columns::filter_for_memory(),
                ),
            ],
            MemoryTable::new(
                memory::columns::data_for_memoryinit(),
                memory::columns::filter_for_memoryinit(),
            ),
        )
    }
}

pub struct BitshiftCpuTable;

impl Lookups for BitshiftCpuTable {
    fn lookups() -> CrossTableLookup {
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

pub struct InnerCpuTable;

impl Lookups for InnerCpuTable {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_inst(),
                Column::single(cpu::columns::col_map().cpu.is_running),
            )],
            CpuTable::new(
                cpu::columns::data_for_permuted_inst(),
                Column::single(cpu::columns::col_map().cpu.is_running),
            ),
        )
    }
}

pub struct ProgramCpuTable;

impl Lookups for ProgramCpuTable {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_permuted_inst(),
                Column::single(cpu::columns::col_map().permuted.filter),
            )],
            ProgramTable::new(
                program::columns::data_for_ctl(),
                Column::single(program::columns::col_map().filter),
            ),
        )
    }
}

pub struct RangeCheckU8LookupTable;
impl Lookups for RangeCheckU8LookupTable {
    fn lookups() -> CrossTableLookup {
        let looking: Vec<Table> = chain![
            rangecheck_looking(),
            memory::columns::rangecheck_u8_looking(),
        ]
        .collect();
        CrossTableLookup::new(
            looking,
            RangeCheckU8Table::new(
                crate::rangecheck_u8::columns::data(),
                crate::rangecheck_u8::columns::filter(),
            ),
        )
    }
}

pub struct HalfWordMemoryCpuTable;

impl Lookups for HalfWordMemoryCpuTable {
    fn lookups() -> CrossTableLookup {
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

pub struct FullWordMemoryCpuTable;

impl Lookups for FullWordMemoryCpuTable {
    fn lookups() -> CrossTableLookup {
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

#[cfg(feature = "enable_register_starks")]
pub struct RegisterLookups;

#[cfg(feature = "enable_register_starks")]
impl Lookups for RegisterLookups {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            chain![
                crate::cpu::columns::register_looking(),
                crate::memory_io::columns::register_looking()
            ]
            .collect(),
            crate::register::columns::register_looked(),
        )
    }
}

#[cfg(feature = "enable_register_starks")]
pub struct RegisterRegInitTable;

#[cfg(feature = "enable_register_starks")]
impl Lookups for RegisterRegInitTable {
    fn lookups() -> CrossTableLookup {
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

pub struct IoMemoryPrivateCpuTable;

impl Lookups for IoMemoryPrivateCpuTable {
    fn lookups() -> CrossTableLookup {
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

pub struct IoMemoryPublicCpuTable;

impl Lookups for IoMemoryPublicCpuTable {
    fn lookups() -> CrossTableLookup {
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

pub struct IoTranscriptCpuTable;

impl Lookups for IoTranscriptCpuTable {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            vec![CpuTable::new(
                cpu::columns::data_for_io_transcript(),
                cpu::columns::filter_for_io_transcript(),
            )],
            IoTranscriptTable::new(
                memory_io::columns::data_for_cpu(),
                memory_io::columns::filter_for_cpu(),
            ),
        )
    }
}

#[cfg(feature = "enable_poseidon_starks")]
pub struct Poseidon2SpongeCpuTable;
#[cfg(feature = "enable_poseidon_starks")]
impl Lookups for Poseidon2SpongeCpuTable {
    fn lookups() -> CrossTableLookup {
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

#[cfg(feature = "enable_poseidon_starks")]
pub struct Poseidon2Poseidon2SpongeTable;
#[cfg(feature = "enable_poseidon_starks")]
impl Lookups for Poseidon2Poseidon2SpongeTable {
    fn lookups() -> CrossTableLookup {
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

#[cfg(feature = "enable_poseidon_starks")]
pub struct Poseidon2OutputBytesPoseidon2SpongeTable;
#[cfg(feature = "enable_poseidon_starks")]
impl Lookups for Poseidon2OutputBytesPoseidon2SpongeTable {
    fn lookups() -> CrossTableLookup {
        CrossTableLookup::new(
            vec![Poseidon2OutputBytesTable::new(
                crate::poseidon2_output_bytes::columns::data_for_poseidon2_sponge(),
                crate::poseidon2_output_bytes::columns::filter_for_poseidon2_sponge(),
            )],
            Poseidon2SpongeTable::new(
                crate::poseidon2_sponge::columns::data_for_poseidon2_output_bytes(),
                crate::poseidon2_sponge::columns::filter_for_poseidon2_output_bytes(),
            ),
        )
    }
}
