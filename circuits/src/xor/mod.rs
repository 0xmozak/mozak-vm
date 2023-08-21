//! This module contains the **`Xor` STARK Table**.
//! This Stark is used to contain the Xor evaluation of the execution.
//! Using the Xor table, we can then construct the other
//! bitwise operations, such as `And` and `Or`.
//! It is used from the CPU STARK with the Cross Table Lookup (CTL) technique.

pub mod columns;
pub mod stark;
