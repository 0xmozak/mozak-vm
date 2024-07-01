use core::fmt::Debug;
use std::fmt::Display;

use super::columns::RegisterZeroWrite;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct RegisterZeroWriteConstraints {}

const COLUMNS: usize = RegisterZeroWrite::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for RegisterZeroWriteConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = RegisterZeroWrite<E>;
}

impl Display for RegisterZeroWriteConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[allow(clippy::module_name_repetitions)]
pub type RegisterZeroWriteStark<F, const D: usize> =
    StarkFrom<F, RegisterZeroWriteConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;
