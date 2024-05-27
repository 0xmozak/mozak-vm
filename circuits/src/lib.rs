#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// TODO: When things have settled a bit, and we make a big push to improve docs, we can remove these
// exceptions:
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![feature(const_trait_impl)]

pub mod bitshift;
pub mod columns_view;
pub mod cpu;
pub mod cpu_skeleton;
pub mod cross_table_lookup;
pub mod expr;
pub mod generation;
pub mod linear_combination;
pub mod linear_combination_typed;
pub mod memory;
pub mod memory_fullword;
pub mod memory_halfword;
pub mod memory_zeroinit;
pub mod memoryinit;
pub mod ops;
pub mod poseidon2;
pub mod poseidon2_output_bytes;
pub mod poseidon2_sponge;
pub mod program;
pub mod program_multiplicities;
pub mod public_sub_table;
pub mod rangecheck;
pub mod rangecheck_u8;
pub mod register;
pub mod stark;
pub mod storage_device;
pub mod tape_commitments;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod unstark;
pub mod utils;
pub mod xor;

extern crate serde;
extern crate serde_big_array;
extern crate serde_derive;
extern crate serde_json;
