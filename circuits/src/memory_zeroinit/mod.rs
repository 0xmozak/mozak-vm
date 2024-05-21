//! This module contains the **`MemoryZeroInit` STARK Table**.
//!
//! This table zero initializes memory addresses which are accessed (through
//! both stores/loads) during execution time in order to circumvent having to
//! require a store before a load for a specific address.
//!
//! Note that this is different from the `MemoryInit` STARK table, which
//! references the memory initialized from the static ELF.
pub mod columns;
pub mod generation;
pub mod stark;
