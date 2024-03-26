#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// TODO: When things have settled a bit, and we make a big push to improve docs, we can remove these
// exceptions:
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![feature(const_trait_impl)]
// Some of our dependencies transitively depend on different versions of the same crates, like syn
// and bitflags. TODO: remove once our dependencies no longer do that.
#![allow(clippy::multiple_crate_versions)]

pub mod bitshift;
pub mod columns_view;
pub mod cpu;
pub mod cross_table_lookup;
pub mod generation;
pub mod linear_combination;
pub mod linear_combination_typed;
pub mod memory;
pub mod memory_fullword;
pub mod memory_halfword;
pub mod memory_io;
pub mod memory_zeroinit;
pub mod memoryinit;
pub mod open_public;
pub mod poseidon2;
pub mod poseidon2_output_bytes;
pub mod poseidon2_sponge;
pub mod program;
pub mod rangecheck;
pub mod rangecheck_u8;
pub mod recproof;
pub mod register;
pub mod registerinit;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod utils;
pub mod xor;
