#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![deny(unsafe_code)]
#![deny(unused_crate_dependencies)]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub mod config;
pub mod node;
