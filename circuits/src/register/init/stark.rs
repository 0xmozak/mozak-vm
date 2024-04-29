use super::columns::RegisterInit;
use crate::columns_view::NumberOfColumns;
use crate::unstark::Unstark;

#[allow(clippy::module_name_repetitions)]
pub type RegisterInitStark<F, const D: usize> =
    Unstark<F, D, RegisterInit<F>, { RegisterInit::<()>::NUMBER_OF_COLUMNS }>;
