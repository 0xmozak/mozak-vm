#![feature(stmt_expr_attributes)]
#![feature(register_tool)]
#![register_tool(tarpaulin)]
#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod decode;
pub mod elf;
pub mod instruction;
pub mod state;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod util;
pub mod vm;

extern crate alloc;
