//! This module contains the **`Register` STARK Table**.
//!
//! This module emulates the 32 registers found in a RISC-V core,
//! indexed by addresses 0..=31 instead.
//!
//! This implementation is very similar to that of the
//! [Memory STARK](crate::memory)
pub mod columns;
pub mod stark;
pub mod general;
pub mod zero_read;
pub mod zero_write;
pub mod init;
