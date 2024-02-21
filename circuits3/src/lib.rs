#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod bitshift;
pub mod columns_view;
pub mod cpu;
pub mod generation;
pub mod utils;
pub mod xor;

#[cfg(any(feature = "cli_bench", test))]
pub mod config;
