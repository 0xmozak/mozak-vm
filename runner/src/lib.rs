#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

// for `syn`
// TODO: remove once it's fixed.
#![allow(clippy::multiple_crate_versions)]

#[cfg(not(target_arch = "wasm32"))]
use mimalloc::MiMalloc;

#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub mod decode;
pub mod ecall;
pub mod elf;
pub mod instruction;
pub mod poseidon2;
pub mod state;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod util;
pub mod vm;

extern crate alloc;
