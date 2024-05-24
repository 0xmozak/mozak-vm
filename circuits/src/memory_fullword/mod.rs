//! This module contains the **`Fullword-Memory` STARK Table**.
//! This Stark is used to store the VM Memory and
//! constrains the load and store operations by the CPU
//! using the CTL (cross table lookup) technique.

pub mod columns;
pub mod generation;
pub mod stark;
