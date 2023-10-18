//! This module contains the underlying STARK cryptography.
//! Docs are still to be added, for now, please refer to notion
//! `doc` section for details.

pub mod lookup;
#[allow(clippy::module_name_repetitions)]
pub mod mozak_stark;
pub mod permutation;
pub mod poly;
pub mod proof;
pub mod prover;
pub mod serde;
pub mod utils;
pub mod verifier;
