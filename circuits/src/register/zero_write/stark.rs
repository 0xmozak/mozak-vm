use super::columns::RegisterZeroWrite;
use crate::columns_view::NumberOfColumns;
use crate::unstark::Unstark;

#[allow(clippy::module_name_repetitions)]
pub type RegisterZeroWriteStark<F, const D: usize> =
    Unstark<F, D, RegisterZeroWrite<F>, { RegisterZeroWrite::<()>::NUMBER_OF_COLUMNS }>;
