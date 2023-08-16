//! This module contains the **`Xor` STARK Table**.
//! This Stark contains the evaluation of Xor for different arguments.
//! Using this Xor table, we can then construct the other
//! bitwise operations, such as `And` and `Or`.
//! It is used from the CPU STARK with the Cross Table Lookup (CTL) technique.

pub mod columns;
pub mod stark;
