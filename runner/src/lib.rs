#![feature(stmt_expr_attributes)]
#![feature(register_tool)]
#![register_tool(tarpaulin)]
#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// FIXME: Remove this, when proptest's macro is updated not to trigger clippy.
#![allow(clippy::ignored_unit_patterns)]
#![deny(unsafe_code)]
#![deny(unused_crate_dependencies)]

pub mod decode;
pub mod elf;
pub mod instruction;
pub mod state;
pub mod system;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod util;
pub mod vm;

extern crate alloc;
