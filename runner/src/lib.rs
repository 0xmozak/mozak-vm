#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

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
extern crate core;
