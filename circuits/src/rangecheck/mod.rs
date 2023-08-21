//! This module contains the **`RangeCheck` STARK Table**.
//! It is used to check that values are between 0 and 2^32 (exclusive),
//! i.e. [0, 2^32).
//!
//! This is done by further splitting the 32-bit value into two 16-bit limbs,
//! and then checking that each limb is in the range 0 through
//! [`RANGE_CHECK_U16_SIZE`](crate::generation::rangecheck::RANGE_CHECK_U16_SIZE)
//! exclusive, i.e. [0..2^16)
//!
//! The STARK is then used by the CPU STARK with the Cross Table Lookup (CTL)
//! technique.

pub mod columns;
pub mod stark;
