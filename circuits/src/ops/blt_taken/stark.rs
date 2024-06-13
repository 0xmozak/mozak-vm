use super::columns::BltTaken;
use crate::columns_view::NumberOfColumns;
use crate::unstark::{impl_name, Unstark};

impl_name!(N, BltTakenStark);

#[allow(clippy::module_name_repetitions)]
pub type BltTakenStark<F, const D: usize> =
    Unstark<F, N, D, BltTaken<F>, { BltTaken::<()>::NUMBER_OF_COLUMNS }>;
