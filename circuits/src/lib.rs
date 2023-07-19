#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(stmt_expr_attributes)]
#![feature(no_coverage)]
#![feature(register_tool)]
#![feature(bigint_helper_methods)]
#![register_tool(tarpaulin)]
#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod bitwise;
pub mod cpu;
pub mod cross_table_lookup;
pub mod generation;
pub mod lookup;
pub mod memory;
pub mod rangecheck;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod utils;
