use super::columns::RegisterZeroWrite;
use crate::columns_view::NumberOfColumns;
use crate::unstark::{impl_name, Unstark};

impl_name!(N, RegisterZeroWriteStark);

#[allow(clippy::module_name_repetitions)]
pub type RegisterZeroWriteStark<F, const D: usize> =
    Unstark<F, N, D, RegisterZeroWrite<F>, { RegisterZeroWrite::<()>::NUMBER_OF_COLUMNS }>;
