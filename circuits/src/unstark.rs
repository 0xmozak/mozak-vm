use core::fmt::Debug;
use std::marker::PhantomData;

use crate::columns_view::columns_view_impl;

/// NoColumns is a PhantomData that supports all the traits and interafaces that
/// we are using for Columns.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct NoColumns<T> {
    _phantom: PhantomData<T>,
}
columns_view_impl!(NoColumns);
