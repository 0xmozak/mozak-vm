use super::columns::RegisterZeroRead;
use crate::columns_view::NumberOfColumns;
use crate::zero_constraints_stark::Unstark;

#[allow(clippy::module_name_repetitions)]
pub type RegisterZeroReadStark<F, const D: usize> =
    Unstark<F, D, RegisterZeroRead<F>, { RegisterZeroRead::<()>::NUMBER_OF_COLUMNS }>;
