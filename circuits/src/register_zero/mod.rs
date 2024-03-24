//! This module contains the **`RegisterZero` STARK Table**.
//!
//! This is a helper for the `Register` STARK Table,
//! to deal with register 0.  Register 0 accepts any writes of any value,
//! but always reads as 0.
pub mod columns;
pub mod stark;
