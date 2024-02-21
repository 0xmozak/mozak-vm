#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

pub mod bitshift;
pub mod columns_view;
pub mod cpu;
pub mod generation;

#[cfg(any(feature = "test", test))]
pub mod config;
