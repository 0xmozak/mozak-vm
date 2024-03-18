#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![deny(unsafe_code)]
#![deny(unused_crate_dependencies)]
// for syn
// TODO: remove once it's fixed.
#![allow(clippy::multiple_crate_versions)]

pub mod block_proposer;
pub mod config;
pub mod node;
