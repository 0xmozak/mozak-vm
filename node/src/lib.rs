#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![deny(unsafe_code)]
#![deny(unused_crate_dependencies)]

use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};

pub mod block_proposer;
pub mod types;

pub const D: usize = 2;
pub type C = Poseidon2GoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;
