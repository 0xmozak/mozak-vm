use std::array::from_fn;
use std::ops::{Index, IndexMut, Neg};
extern crate serde;
extern crate serde_json;
use cpu::columns::CpuState;
use itertools::{chain, izip};
use mozak_circuits_derive::StarkSet;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
#[allow(clippy::wildcard_imports)]
use plonky2_maybe_rayon::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use crate::bitshift::columns::{Bitshift, BitshiftView};
use crate::bitshift::stark::BitshiftStark;
use crate::columns_view::columns_view_impl;
use crate::cpu::stark::CpuStark;
use crate::cpu_skeleton::columns::{CpuSkeleton, CpuSkeletonCtl};
use crate::cpu_skeleton::stark::CpuSkeletonStark;
use crate::cross_table_lookup::{
    Column, ColumnWithTypedInput, CrossTableLookup, CrossTableLookupWithTypedOutput,
};
use crate::memory::columns::{Memory, MemoryCtl};
use crate::memory::stark::MemoryStark;
use crate::memory_halfword::columns::HalfWordMemory;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memory_zeroinit::columns::MemoryZeroInit;
use crate::memory_zeroinit::stark::MemoryZeroInitStark;
use crate::memoryinit::columns::{MemoryInit, MemoryInitCtl};
use crate::memoryinit::stark::MemoryInitStark;
use crate::ops::add::columns::Add;
use crate::ops::add::stark::AddStark;
use crate::ops::blt_taken::columns::BltTaken;
use crate::ops::blt_taken::stark::BltTakenStark;
use crate::ops::lw::columns::LoadWord;
use crate::ops::lw::stark::LoadWordStark;
use crate::ops::sw::columns::StoreWord;
use crate::ops::sw::stark::StoreWordStark;
use crate::poseidon2::columns::{Poseidon2State, Poseidon2StateCtl};
use crate::poseidon2::stark::Poseidon2_12Stark;
use crate::poseidon2_output_bytes::columns::{Poseidon2OutputBytes, Poseidon2OutputBytesCtl};
use crate::poseidon2_output_bytes::stark::Poseidon2OutputBytesStark;
use crate::poseidon2_sponge::columns::{Poseidon2Sponge, Poseidon2SpongeCtl};
use crate::poseidon2_sponge::stark::Poseidon2SpongeStark;
use crate::program::columns::ProgramRom;
use crate::program::stark::ProgramStark;
use crate::program_multiplicities::columns::ProgramMult;
use crate::program_multiplicities::stark::ProgramMultStark;
use crate::public_sub_table::PublicSubTable;
use crate::rangecheck::columns::{rangecheck_looking, RangeCheckColumnsView, RangeCheckCtl};
use crate::rangecheck::stark::RangeCheckStark;
use crate::rangecheck_u8::columns::RangeCheckU8;
use crate::rangecheck_u8::stark::RangeCheckU8Stark;
use crate::register::general::columns::Register;
use crate::register::general::stark::RegisterStark;
use crate::register::init::columns::RegisterInit;
use crate::register::init::stark::RegisterInitStark;
use crate::register::zero_read::columns::RegisterZeroRead;
use crate::register::zero_read::stark::RegisterZeroReadStark;
use crate::register::zero_write::columns::RegisterZeroWrite;
use crate::register::zero_write::stark::RegisterZeroWriteStark;
use crate::register::RegisterCtl;
use crate::storage_device::columns::{StorageDevice, StorageDeviceCtl};
use crate::storage_device::stark::StorageDeviceStark;
use crate::tape_commitments::columns::{TapeCommitmentCTL, TapeCommitments};
use crate::tape_commitments::stark::TapeCommitmentsStark;
use crate::xor::columns::{XorColumnsView, XorView};
use crate::xor::stark::XorStark;
use crate::{
    bitshift, cpu, cpu_skeleton, memory, memory_halfword, memory_zeroinit, memoryinit, ops,
    poseidon2_output_bytes, poseidon2_sponge, program, program_multiplicities, rangecheck,
    register, storage_device, xor,
};

const NUM_CROSS_TABLE_LOOKUP: usize = 17;
const NUM_PUBLIC_SUB_TABLES: usize = 2;

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
    #[StarkSet(stark_kind = "ProgramMult")]
    pub program_mult_stark: ProgramMultStark<F, D>,
    #[StarkSet(stark_kind = "Memory")]
    pub memory_stark: MemoryStark<F, D>,
    #[StarkSet(stark_kind = "ElfMemoryInit")]
    pub elf_memory_init_stark: MemoryInitStark<F, D>,
    #[StarkSet(stark_kind = "CallTapeInit")]
    pub call_tape_init_stark: MemoryInitStark<F, D>,
    #[StarkSet(stark_kind = "PrivateTapeInit")]
    pub private_tape_init_stark: MemoryInitStark<F, D>,
    #[StarkSet(stark_kind = "PublicTapeInit")]
    pub public_tape_init_stark: MemoryInitStark<F, D>,
    #[StarkSet(stark_kind = "EventTapeInit")]
    pub event_tape_init_stark: MemoryInitStark<F, D>,
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
    #[StarkSet(stark_kind = "StorageDevicePrivate")]
    pub private_tape_stark: StorageDeviceStark<F, D>,
    #[StarkSet(stark_kind = "StorageDevicePublic")]
    pub public_tape_stark: StorageDeviceStark<F, D>,
    #[StarkSet(stark_kind = "CallTape")]
    pub call_tape_stark: StorageDeviceStark<F, D>,
    #[StarkSet(stark_kind = "EventTape")]
    pub event_tape_stark: StorageDeviceStark<F, D>,
    // TODO(bing): This is known to be 32-bytes in length. Optimize with
    // a fixed size version of this STARK.
    #[StarkSet(stark_kind = "EventsCommitmentTape")]
    pub events_commitment_tape_stark: StorageDeviceStark<F, D>,
    // TODO(bing): This is known to be 32-bytes in length. Optimize with
    // a fixed size version of this STARK.
    #[StarkSet(stark_kind = "CastListCommitmentTape")]
    pub cast_list_commitment_tape_stark: StorageDeviceStark<F, D>,
    #[StarkSet(stark_kind = "RegisterInit")]
    pub register_init_stark: RegisterInitStark<F, D>,
    #[StarkSet(stark_kind = "Register")]
    pub register_stark: RegisterStark<F, D>,
    #[StarkSet(stark_kind = "RegisterZeroRead")]
    pub register_zero_read_stark: RegisterZeroReadStark<F, D>,
    #[StarkSet(stark_kind = "RegisterZeroWrite")]
    pub register_zero_write_stark: RegisterZeroWriteStark<F, D>,
    #[StarkSet(stark_kind = "Poseidon2")]
    pub poseidon2_stark: Poseidon2_12Stark<F, D>,
    #[StarkSet(stark_kind = "Poseidon2Sponge")]
    pub poseidon2_sponge_stark: Poseidon2SpongeStark<F, D>,
    #[StarkSet(stark_kind = "Poseidon2OutputBytes")]
    pub poseidon2_output_bytes_stark: Poseidon2OutputBytesStark<F, D>,
    #[StarkSet(stark_kind = "CpuSkeleton")]
    pub cpu_skeleton_stark: CpuSkeletonStark<F, D>,
    #[StarkSet(stark_kind = "Add")]
    pub add_stark: AddStark<F, D>,
    #[StarkSet(stark_kind = "BltTaken")]
    pub blt_taken_stark: BltTakenStark<F, D>,
    #[StarkSet(stark_kind = "StoreWord")]
    pub store_word_stark: StoreWordStark<F, D>,
    #[StarkSet(stark_kind = "LoadWord")]
    pub load_word_stark: LoadWordStark<F, D>,
    #[StarkSet(stark_kind = "TapeCommitments")]
    pub tape_commitments_stark: TapeCommitmentsStark<F, D>,
    pub cross_table_lookups: [CrossTableLookup; NUM_CROSS_TABLE_LOOKUP],
    pub public_sub_tables: [PublicSubTable; NUM_PUBLIC_SUB_TABLES],
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

        /// Creates an array by repeatedly calls a "lambda" once per stark type.
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

        #[allow(unused_macros)]
        macro_rules! all_starks_par {
            ($all_stark:expr, |$stark:ident, $kind:ident| $val:expr) => {{
                use core::borrow::Borrow;
                use $crate::stark::mozak_stark::{TableKindArray, TableKind::*};
                let all_stark = $all_stark.borrow();
                TableKindArray([$(
                    {
                        let $stark = &all_stark.$fields;
                        let $kind = $kind_names;
                        let f: Box<dyn Fn() -> _ + Send + Sync> = Box::new(move || $val);
                        f
                    },)*
                ]).par_map(|f| f())
            }};
        }
        #[allow(unused_imports)]
        pub(crate) use all_starks_par;
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
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct TableKindArray<T>(#[serde(with = "BigArray")] pub [T; TableKind::COUNT]);

impl<T: Default> Default for TableKindArray<T> {
    fn default() -> Self { TableKindArray(from_fn(|_| T::default())) }
}

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

impl<T: Send> TableKindArray<T> {
    pub fn par_map<F, U>(self, f: F) -> TableKindArray<U>
    where
        F: Fn(T) -> U + Send + Sync,
        U: Send + core::fmt::Debug, {
        TableKindArray(
            self.0
                .into_par_iter()
                .map(f)
                .collect::<Vec<U>>()
                .try_into()
                .unwrap(),
        )
    }
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

    pub fn each_ref(&self) -> TableKindArray<&T> { TableKindArray(self.0.each_ref()) }

    pub fn iter(&self) -> impl Iterator<Item = &T> { self.0.iter() }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> { self.0.iter_mut() }
}

columns_view_impl!(PublicInputs);

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize)]
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
            program_mult_stark: ProgramMultStark::default(),
            memory_stark: MemoryStark::default(),
            elf_memory_init_stark: MemoryInitStark::default(),
            call_tape_init_stark: MemoryInitStark::default(),
            private_tape_init_stark: MemoryInitStark::default(),
            public_tape_init_stark: MemoryInitStark::default(),
            event_tape_init_stark: MemoryInitStark::default(),
            mozak_memory_init_stark: MemoryInitStark::default(),
            memory_zeroinit_stark: MemoryZeroInitStark::default(),
            rangecheck_u8_stark: RangeCheckU8Stark::default(),
            halfword_memory_stark: HalfWordMemoryStark::default(),
            register_init_stark: RegisterInitStark::default(),
            register_stark: RegisterStark::default(),
            register_zero_read_stark: RegisterZeroReadStark::default(),
            register_zero_write_stark: RegisterZeroWriteStark::default(),
            private_tape_stark: StorageDeviceStark::default(),
            public_tape_stark: StorageDeviceStark::default(),
            call_tape_stark: StorageDeviceStark::default(),
            event_tape_stark: StorageDeviceStark::default(),
            events_commitment_tape_stark: StorageDeviceStark::default(),
            cast_list_commitment_tape_stark: StorageDeviceStark::default(),
            poseidon2_sponge_stark: Poseidon2SpongeStark::default(),
            poseidon2_stark: Poseidon2_12Stark::default(),
            poseidon2_output_bytes_stark: Poseidon2OutputBytesStark::default(),
            cpu_skeleton_stark: CpuSkeletonStark::default(),
            add_stark: AddStark::default(),
            blt_taken_stark: BltTakenStark::default(),
            store_word_stark: StoreWordStark::default(),
            load_word_stark: LoadWordStark::default(),
            tape_commitments_stark: TapeCommitmentsStark::default(),

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
                RegisterLookups::lookups(),
                StorageDeviceToCpuTable::lookups(),
                Poseidon2SpongeCpuTable::lookups(),
                Poseidon2Poseidon2SpongeTable::lookups(),
                Poseidon2OutputBytesPoseidon2SpongeTable::lookups(),
                CpuToSkeletonTable::lookups(),
                EventCommitmentTapeIOLookupTable::lookups(),
                CastlistCommitmentTapeIOLookupTable::lookups(),
            ],
            public_sub_tables: [
                crate::tape_commitments::columns::make_event_commitment_tape_public(),
                crate::tape_commitments::columns::make_castlist_commitment_tape_public(),
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

#[derive(Debug, Clone, Copy)]
pub struct TableWithTypedInputAndOutput<Row, Filter> {
    pub(crate) kind: TableKind,
    pub(crate) columns: Row,
    pub(crate) filter_column: Filter,
}

impl<RowIn, RowOut, I> From<TableWithTypedInputAndOutput<RowIn, ColumnWithTypedInput<I>>>
    for TableWithTypedOutput<RowOut>
where
    I: IntoIterator<Item = i64>,
    RowOut: FromIterator<Column>,
    RowIn: IntoIterator<Item = ColumnWithTypedInput<I>>,
{
    fn from(input: TableWithTypedInputAndOutput<RowIn, ColumnWithTypedInput<I>>) -> Self {
        TableWithTypedOutput {
            kind: input.kind,
            columns: input.columns.into_iter().map(Column::from).collect(),
            filter_column: input.filter_column.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableWithTypedOutput<Row> {
    // TODO: when converting to untyped table, check that TableKind agrees with columns type.
    // That would have prevented some mistakes.
    pub(crate) kind: TableKind,
    pub(crate) columns: Row,
    pub(crate) filter_column: Column,
}

pub type TableUntyped = TableWithTypedOutput<Vec<Column>>;
pub use TableUntyped as Table;

impl<Row: IntoIterator<Item = Column>> TableWithTypedOutput<Row> {
    pub fn to_untyped_output(self) -> Table {
        Table {
            kind: self.kind,
            columns: self.columns.into_iter().collect(),
            filter_column: self.filter_column,
        }
    }
}

impl<Row> TableWithTypedOutput<Row> {
    pub fn new(kind: TableKind, columns: Row, filter_column: Column) -> Self {
        Self {
            kind,
            columns,
            filter_column,
        }
    }
}

impl<Row> Neg for TableWithTypedOutput<Row> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::Output {
            kind: self.kind,
            columns: self.columns,
            filter_column: -self.filter_column,
        }
    }
}

/// Macro to instantiate a new table for cross table lookups.
// OK, `table_kind` determines the input type of the table.
// But input type could relate to multiple kinds.
macro_rules! table_impl {
    ($lookup_input_id: ident, $table_kind: expr, $input_table_type: ident) => {
        #[allow(non_snake_case)]
        pub mod $lookup_input_id {
            use super::*;
            pub fn new<RowIn, RowOut>(
                columns: RowIn,
                filter_column: ColumnWithTypedInput<$input_table_type<i64>>,
            ) -> TableWithTypedOutput<RowOut>
            where
                RowOut: FromIterator<Column>,
                RowIn: IntoIterator<Item = ColumnWithTypedInput<$input_table_type<i64>>>, {
                TableWithTypedOutput {
                    kind: $table_kind,
                    columns: columns.into_iter().map(Column::from).collect(),
                    filter_column: filter_column.into(),
                }
            }
        }
    };
}

// TODO(Matthias): The information provided in the macro invocations here is
// already present in `#[StarkSet(macro_name = "mozak_stark_set")]`, so we could
// potentially generate these.
table_impl!(
    RangeCheckTable,
    TableKind::RangeCheck,
    RangeCheckColumnsView
);
table_impl!(CpuTable, TableKind::Cpu, CpuState);
table_impl!(XorTable, TableKind::Xor, XorColumnsView);
table_impl!(BitshiftTable, TableKind::Bitshift, BitshiftView);
table_impl!(ProgramTable, TableKind::Program, ProgramRom);
table_impl!(ProgramMultTable, TableKind::ProgramMult, ProgramMult);
table_impl!(MemoryTable, TableKind::Memory, Memory);
table_impl!(ElfMemoryInitTable, TableKind::ElfMemoryInit, MemoryInit);
table_impl!(CallTapeInitTable, TableKind::CallTapeInit, MemoryInit);
table_impl!(PrivateTapeInitTable, TableKind::PrivateTapeInit, MemoryInit);
table_impl!(PublicTapeInitTable, TableKind::PublicTapeInit, MemoryInit);
table_impl!(EventTapeInitTable, TableKind::EventTapeInit, MemoryInit);
table_impl!(MozakMemoryInitTable, TableKind::MozakMemoryInit, MemoryInit);
table_impl!(
    MemoryZeroInitTable,
    TableKind::MemoryZeroInit,
    MemoryZeroInit
);
table_impl!(RangeCheckU8Table, TableKind::RangeCheckU8, RangeCheckU8);
table_impl!(
    HalfWordMemoryTable,
    TableKind::HalfWordMemory,
    HalfWordMemory
);
table_impl!(RegisterInitTable, TableKind::RegisterInit, RegisterInit);
table_impl!(RegisterTable, TableKind::Register, Register);
table_impl!(
    RegisterZeroReadTable,
    TableKind::RegisterZeroRead,
    RegisterZeroRead
);
table_impl!(
    RegisterZeroWriteTable,
    TableKind::RegisterZeroWrite,
    RegisterZeroWrite
);
table_impl!(
    StorageDevicePrivateTable,
    TableKind::StorageDevicePrivate,
    StorageDevice
);
table_impl!(
    StorageDevicePublicTable,
    TableKind::StorageDevicePublic,
    StorageDevice
);
table_impl!(CallTapeTable, TableKind::CallTape, StorageDevice);
table_impl!(EventTapeTable, TableKind::EventTape, StorageDevice);
table_impl!(
    EventsCommitmentTapeTable,
    TableKind::EventsCommitmentTape,
    StorageDevice
);
table_impl!(
    CastListCommitmentTapeTable,
    TableKind::CastListCommitmentTape,
    StorageDevice
);
table_impl!(
    Poseidon2SpongeTable,
    TableKind::Poseidon2Sponge,
    Poseidon2Sponge
);
table_impl!(
    TapeCommitmentsTable,
    TableKind::TapeCommitments,
    TapeCommitments
);
table_impl!(Poseidon2Table, TableKind::Poseidon2, Poseidon2State);
table_impl!(
    Poseidon2OutputBytesTable,
    TableKind::Poseidon2OutputBytes,
    Poseidon2OutputBytes
);
table_impl!(SkeletonTable, TableKind::CpuSkeleton, CpuSkeleton);
table_impl!(AddTable, TableKind::Add, Add);
table_impl!(BltTakenTable, TableKind::BltTaken, BltTaken);
table_impl!(StoreWordTable, TableKind::StoreWord, StoreWord);
table_impl!(LoadWordTable, TableKind::LoadWord, LoadWord);

pub trait Lookups {
    type Row: IntoIterator<Item = Column>;
    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row>;
    #[must_use]
    fn lookups() -> CrossTableLookup { Self::lookups_with_typed_output().to_untyped_output() }
}

pub struct CpuToSkeletonTable;

impl Lookups for CpuToSkeletonTable {
    type Row = CpuSkeletonCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![
                cpu::columns::lookup_for_skeleton(),
                ops::add::columns::lookup_for_skeleton(),
                ops::blt_taken::columns::lookup_for_skeleton(),
                ops::sw::columns::lookup_for_skeleton(),
                ops::lw::columns::lookup_for_skeleton(),
            ],
            vec![cpu_skeleton::columns::lookup_for_cpu()],
        )
    }
}

pub struct RangecheckTable;

impl Lookups for RangecheckTable {
    type Row = RangeCheckCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        let register = register::general::columns::rangecheck_looking();

        let looking: Vec<TableWithTypedOutput<_>> = chain![
            memory::columns::rangecheck_looking(),
            cpu::columns::rangecheck_looking(),
            ops::add::columns::rangecheck_looking(),
            ops::sw::columns::rangecheck_looking(),
            // TODO(Matthias):
            ops::lw::columns::rangecheck_looking(),
            register,
        ]
        .collect();
        CrossTableLookupWithTypedOutput::new(looking, vec![rangecheck::columns::lookup()])
    }
}

pub struct XorCpuTable;

impl Lookups for XorCpuTable {
    type Row = XorView<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(vec![cpu::columns::lookup_for_xor()], vec![
            xor::columns::lookup_for_cpu(),
        ])
    }
}

pub struct IntoMemoryTable;

impl Lookups for IntoMemoryTable {
    type Row = MemoryCtl<Column>;

    #[allow(clippy::too_many_lines)]
    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        let tables = chain![
            [cpu::columns::lookup_for_memory()],
            [
                TableKind::StorageDevicePrivate,
                TableKind::StorageDevicePublic,
                TableKind::CallTape,
                TableKind::EventTape,
                TableKind::EventsCommitmentTape,
                TableKind::CastListCommitmentTape
            ]
            .map(storage_device::columns::lookup_for_memory),
            ops::sw::columns::lookup_for_memory_limb(),
            ops::lw::columns::lookup_for_memory_limb(),
            memory_halfword::columns::lookup_for_memory_limb(),
            poseidon2_sponge::columns::lookup_for_input_memory(),
            poseidon2_output_bytes::columns::lookup_for_output_memory(),
        ]
        .collect();
        CrossTableLookupWithTypedOutput::new(tables, vec![memory::columns::lookup_for_cpu()])
    }
}

pub struct MemoryInitMemoryTable;

impl Lookups for MemoryInitMemoryTable {
    type Row = MemoryInitCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<MemoryInitCtl<Column>> {
        CrossTableLookupWithTypedOutput::new(
            vec![
                memoryinit::columns::lookup_for_memory(ElfMemoryInitTable::new),
                memoryinit::columns::lookup_for_memory(MozakMemoryInitTable::new),
                memoryinit::columns::lookup_for_memory(CallTapeInitTable::new),
                memoryinit::columns::lookup_for_memory(PrivateTapeInitTable::new),
                memoryinit::columns::lookup_for_memory(PublicTapeInitTable::new),
                memoryinit::columns::lookup_for_memory(EventTapeInitTable::new),
                memory_zeroinit::columns::lookup_for_memory(),
            ],
            vec![memory::columns::lookup_for_memoryinit()],
        )
    }
}

pub struct BitshiftCpuTable;

impl Lookups for BitshiftCpuTable {
    type Row = Bitshift<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Bitshift<Column>> {
        CrossTableLookupWithTypedOutput::new(vec![cpu::columns::lookup_for_shift_amount()], vec![
            bitshift::columns::lookup_for_cpu(),
        ])
    }
}

pub struct InnerCpuTable;

impl Lookups for InnerCpuTable {
    type Row = ProgramRom<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![
                ops::add::columns::lookup_for_program_rom(),
                ops::blt_taken::columns::lookup_for_program_rom(),
                ops::sw::columns::lookup_for_program_rom(),
                ops::lw::columns::lookup_for_program_rom(),
                cpu::columns::lookup_for_program_rom(),
            ],
            vec![program_multiplicities::columns::lookup_for_cpu()],
        )
    }
}

pub struct ProgramCpuTable;

impl Lookups for ProgramCpuTable {
    type Row = ProgramRom<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![program_multiplicities::columns::lookup_for_rom()],
            vec![program::columns::lookup_for_ctl()],
        )
    }
}

pub struct RangeCheckU8LookupTable;
impl Lookups for RangeCheckU8LookupTable {
    type Row = RangeCheckCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        let looking: Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> = chain![
            rangecheck_looking(),
            memory::columns::rangecheck_u8_looking(),
        ]
        .collect();
        CrossTableLookupWithTypedOutput::new(looking, vec![crate::rangecheck_u8::columns::lookup()])
    }
}

pub struct HalfWordMemoryCpuTable;

impl Lookups for HalfWordMemoryCpuTable {
    type Row = MemoryCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<MemoryCtl<Column>> {
        CrossTableLookupWithTypedOutput::new(
            vec![cpu::columns::lookup_for_halfword_memory()],
            vec![memory_halfword::columns::lookup_for_cpu()],
        )
    }
}

pub struct RegisterLookups;

impl Lookups for RegisterLookups {
    type Row = RegisterCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            chain![
                crate::cpu::columns::register_looking(),
                ops::add::columns::register_looking(),
                ops::blt_taken::columns::register_looking(),
                ops::sw::columns::register_looking(),
                ops::lw::columns::register_looking(),
                crate::storage_device::columns::register_looking(),
                crate::poseidon2_sponge::columns::register_looking(),
                vec![crate::register::init::columns::lookup_for_register()],
            ]
            .collect(),
            vec![
                crate::register::general::columns::register_looked(),
                crate::register::zero_read::columns::register_looked(),
                crate::register::zero_write::columns::register_looked(),
            ],
        )
    }
}

pub struct StorageDeviceToCpuTable;

impl Lookups for StorageDeviceToCpuTable {
    type Row = StorageDeviceCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            izip!(
                [
                    TableKind::StorageDevicePrivate,
                    TableKind::StorageDevicePublic,
                    TableKind::CallTape,
                    TableKind::EventTape,
                    TableKind::EventsCommitmentTape,
                    TableKind::CastListCommitmentTape,
                ],
                0..
            )
            .map(|(kind, i)| storage_device::columns::lookup_for_cpu(kind, i))
            .collect(),
            vec![cpu::columns::lookup_for_storage_tables()],
        )
    }
}

pub struct Poseidon2SpongeCpuTable;

impl Lookups for Poseidon2SpongeCpuTable {
    type Row = Poseidon2SpongeCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![crate::poseidon2_sponge::columns::lookup_for_cpu()],
            vec![crate::cpu::columns::lookup_for_poseidon2_sponge()],
        )
    }
}

pub struct Poseidon2Poseidon2SpongeTable;

impl Lookups for Poseidon2Poseidon2SpongeTable {
    type Row = Poseidon2StateCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![crate::poseidon2::columns::lookup_for_sponge()],
            vec![crate::poseidon2_sponge::columns::lookup_for_poseidon2()],
        )
    }
}

pub struct Poseidon2OutputBytesPoseidon2SpongeTable;

impl Lookups for Poseidon2OutputBytesPoseidon2SpongeTable {
    type Row = Poseidon2OutputBytesCtl<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![crate::poseidon2_output_bytes::columns::lookup_for_poseidon2_sponge()],
            vec![crate::poseidon2_sponge::columns::lookup_for_poseidon2_output_bytes()],
        )
    }
}

pub struct EventCommitmentTapeIOLookupTable;

impl Lookups for EventCommitmentTapeIOLookupTable {
    type Row = TapeCommitmentCTL<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![crate::storage_device::columns::event_commitment_lookup_in_tape_commitments()],
            vec![crate::tape_commitments::columns::lookup_for_event_tape_commitment()],
        )
    }
}

pub struct CastlistCommitmentTapeIOLookupTable;

impl Lookups for CastlistCommitmentTapeIOLookupTable {
    type Row = TapeCommitmentCTL<Column>;

    fn lookups_with_typed_output() -> CrossTableLookupWithTypedOutput<Self::Row> {
        CrossTableLookupWithTypedOutput::new(
            vec![crate::storage_device::columns::castlist_commitment_lookup_in_tape_commitments()],
            vec![crate::tape_commitments::columns::lookup_for_castlist_commitment()],
        )
    }
}
