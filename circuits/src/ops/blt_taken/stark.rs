use super::columns::BltTaken;
use crate::columns_view::NumberOfColumns;
use crate::unstark::Unstark;

#[allow(clippy::module_name_repetitions)]
pub type BltTakenStark<F, const D: usize> =
    Unstark<F, D, BltTaken<F>, { BltTaken::<()>::NUMBER_OF_COLUMNS }>;
