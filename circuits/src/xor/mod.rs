//! This module contains the **`XOR` STARK Table**.
//! This STARK contains the evaluation of XOR for different arguments.
//! Using this XOR table, we can then construct the other
//! bitwise operations, such as `AND` and `OR`.
//! It is used from the CPU STARK with the Cross Table Lookup (CTL) technique.

pub mod columns;
pub mod generation;
pub mod stark;
