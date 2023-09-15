#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(stmt_expr_attributes)]
#![feature(no_coverage)]
#![feature(register_tool)]
#![feature(bigint_helper_methods)]
#![register_tool(tarpaulin)]
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
pub mod limbs;
pub mod linear_combination;
pub mod lookup;
pub mod memory;
pub mod memoryinit;
pub mod program;
pub mod rangecheck;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod utils;
pub mod xor;
