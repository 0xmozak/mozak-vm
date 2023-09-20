//! This module contains the **`RegisterInit` STARK Table**.
//! It stores:
//! 1) register 'addresses', and
//! 2) initialized values,
//!
//! of our emulated RISC-V registers referenced by the Register STARK.
//!
//! This implementation is very similar to that of the
//! [Memory STARK](crate::memory)
//!
//! TODO: update this comment when Register STARK is done
//! Note that this STARK acts as an auxiliary STARK to the
//! Register STARK, which a register file.
pub mod columns;
pub mod stark;
