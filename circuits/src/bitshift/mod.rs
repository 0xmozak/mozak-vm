//! This module contains the **`BitShift` STARK Table**.
//! This Stark is used to constrain the 2^n multiplier values
//! for `n` in the range `0..32`, which correspond to multipliers
//! for the `SHL` and `SHR` VM operations of `n` bits.
//! It is used from the CPU STARK with the Cross Table Lookup (CTL) technique.

pub mod columns;
pub mod generation;
pub mod stark;
