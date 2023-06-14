#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod decode;
pub mod elf;
pub mod instruction;
pub mod state;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod trace;
pub mod util;
pub mod vm;

extern crate alloc;
