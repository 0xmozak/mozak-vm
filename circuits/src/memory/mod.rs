//! This module contains the **`Memory` STARK Table**.
//! It stores the program memory, referenced by the CPU STARK.
pub mod columns;
pub mod stark;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod trace;
