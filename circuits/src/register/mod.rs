//! This module contains the **`Register` STARK Table**.
//!
//! This module emulates the 32 registers found in a RISC-V core,
//! indexed by addresses 0..=31 instead.
//!
//! This implementation is very similar to that of the
//! [Memory STARK](crate::memory)

use crate::columns_view::columns_view_impl;
pub mod general;
pub mod generation;
pub mod init;
pub mod zero_read;
pub mod zero_write;

columns_view_impl!(RegisterCtl);
#[allow(clippy::module_name_repetitions)]
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct RegisterCtl<T> {
    pub clk: T,
    pub op: T,
    pub addr: T,
    pub value: T,
}
