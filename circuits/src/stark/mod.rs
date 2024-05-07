//! This module contains the underlying STARK cryptography.
//! Docs are still to be added, for now, please refer to notion
//! `doc` section for details.

pub mod batch_prover;
pub mod batch_verifier;
#[allow(clippy::module_name_repetitions)]
pub mod mozak_stark;
pub mod permutation;
pub mod proof;
pub mod prover;
pub mod recursive_verifier;
pub mod utils;
pub mod verifier;
