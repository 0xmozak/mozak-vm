#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod cpu;
pub mod generation;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod utils;
