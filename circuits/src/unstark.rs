use core::fmt::Debug;
use std::marker::PhantomData;

use crate::columns_view::columns_view_impl;

/// `NoColumns` is a `PhantomData` that supports all the traits and interafaces
/// that we are using for Columns.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct NoColumns<T> {
    _phantom: PhantomData<T>,
}
columns_view_impl!(NoColumns);

macro_rules! unstark {
    ($name:ident, $view:ty) => {
        type View<T> = $view;

        mod constraints {
            use super::View;
            use crate::columns_view::NumberOfColumns;

            #[derive(Default, Clone, Copy, Debug)]
            pub struct $name {}

            pub const COLUMNS: usize = View::<()>::NUMBER_OF_COLUMNS;
            pub const PUBLIC_INPUTS: usize = 0;

            impl crate::expr::GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for $name {
                type PublicInputs<E: core::fmt::Debug> = crate::unstark::NoColumns<E>;
                type View<E: core::fmt::Debug> = View<E>;
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
        }

        pub type $name<F, const D: usize> = crate::expr::StarkFrom<
            F,
            constraints::$name,
            { D },
            { constraints::COLUMNS },
            { constraints::PUBLIC_INPUTS },
        >;
    };
}

pub(crate) use unstark;
