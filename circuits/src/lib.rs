#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// TODO: When things have settled a bit, and we make a big push to improve docs, we can remove these
// exceptions:
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
// FIXME: Remove this, when proptest's macro is updated not to trigger clippy.
#![allow(clippy::ignored_unit_patterns)]

pub mod bitshift;
pub mod columns_view;
pub mod cpu;
pub mod cross_table_lookup;
pub mod generation;
pub mod linear_combination;
pub mod memory;
pub mod memory_fullword;
pub mod memory_halfword;
pub mod memoryinit;
pub mod poseidon2;
pub mod poseidon2_sponge;
pub mod program;
pub mod rangecheck;
pub mod rangecheck_limb;
pub mod register;
pub mod registerinit;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod utils;
pub mod xor;
