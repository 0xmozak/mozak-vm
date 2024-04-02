#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![deny(unsafe_code)]
#![deny(unused_crate_dependencies)]
// Some of our dependencies transitively depend on different versions of the same crates, like syn
// and bitflags. TODO: remove once our dependencies no longer do that.
#![allow(clippy::multiple_crate_versions)]

pub mod block_proposer;
pub mod config;
pub mod node;
pub mod types;
