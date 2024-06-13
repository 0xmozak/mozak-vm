use super::columns::RegisterZeroRead;
use crate::columns_view::NumberOfColumns;
use crate::unstark::{impl_name, Unstark};

impl_name!(N, RegisterZeroReadStark);

#[allow(clippy::module_name_repetitions)]
pub type RegisterZeroReadStark<F, const D: usize> =
    Unstark<F, N, D, RegisterZeroRead<F>, { RegisterZeroRead::<()>::NUMBER_OF_COLUMNS }>;
