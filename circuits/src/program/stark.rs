use super::columns::ProgramRom;
use crate::columns_view::NumberOfColumns;
use crate::unstark::{impl_name, Unstark};

impl_name!(N, ProgramStark);

#[allow(clippy::module_name_repetitions)]
pub type ProgramStark<F, const D: usize> =
    Unstark<F, N, D, ProgramRom<F>, { ProgramRom::<()>::NUMBER_OF_COLUMNS }>;
