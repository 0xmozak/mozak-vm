//! This module contains the ** `IO-Memory` STARK Table**.
//! This Stark is used to store the VM IO Memory and
//! constrains the load and store operations by the CPU
//! using the CTL (cross table lookup) technique.

pub mod columns;
pub mod stark;
