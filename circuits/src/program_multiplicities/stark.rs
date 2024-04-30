use super::columns::ProgramMult;
use crate::columns_view::NumberOfColumns;
use crate::unstark::Unstark;

#[allow(clippy::module_name_repetitions)]
pub type ProgramMultStark<F, const D: usize> =
    Unstark<F, D, ProgramMult<F>, { ProgramMult::<()>::NUMBER_OF_COLUMNS }>;
