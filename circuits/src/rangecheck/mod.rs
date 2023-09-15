//! This module contains the **`RangeCheck` STARK Table**.
//! It is used to check that values are between 0 and 2^32 (exclusive),
//! i.e. [0, 2^32).
//!
//! This is done by further splitting the 32-bit value into four 8-bit limbs,
//! and then checking that each limb is in the range 0 through 255 (inclusive),
//!
//! The STARK is then used by the CPU STARK with the Cross Table Lookup (CTL)
//! technique.

pub mod columns;
pub mod stark;
