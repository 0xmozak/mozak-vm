use super::columns::ProgramRom;
use crate::columns_view::NumberOfColumns;
use crate::unstark::Unstark;

#[allow(clippy::module_name_repetitions)]
pub type ProgramStark<F, const D: usize> =
    Unstark<F, D, ProgramRom<F>, { ProgramRom::<()>::NUMBER_OF_COLUMNS }>;
