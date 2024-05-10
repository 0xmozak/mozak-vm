//! This module contains the **`Memory` STARK Table**.
//! This Stark is used to store the VM Memory and
//! constrains the load and store operations by the CPU
//! using the CTL (cross table lookup) technique.

pub mod columns;
pub mod fullword;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod trace;
