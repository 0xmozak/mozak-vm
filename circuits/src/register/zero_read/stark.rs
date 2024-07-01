use core::fmt::Debug;
use std::fmt::Display;

use super::columns::RegisterZeroRead;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct RegisterZeroReadConstraints {}

const COLUMNS: usize = RegisterZeroRead::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for RegisterZeroReadConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = RegisterZeroRead<E>;
}

impl Display for RegisterZeroReadConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[allow(clippy::module_name_repetitions)]
pub type RegisterZeroReadStark<F, const D: usize> =
    StarkFrom<F, RegisterZeroReadConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;
