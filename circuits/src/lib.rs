#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(specialization)]
#![feature(stmt_expr_attributes)]
#![feature(no_coverage)]
#![feature(register_tool)]
#![register_tool(tarpaulin)]
#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod bitwise;
pub mod cpu;
pub mod generation;
pub mod lookup;
pub mod memory;
pub mod rangecheck;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod utils;
