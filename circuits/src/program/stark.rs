use core::fmt::Debug;
use std::fmt::Display;

use super::columns::ProgramRom;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct ProgramConstraints {}

const COLUMNS: usize = ProgramRom::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for ProgramConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = ProgramRom<E>;
}

impl Display for ProgramConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[allow(clippy::module_name_repetitions)]
pub type ProgramStark<F, const D: usize> =
    StarkFrom<F, ProgramConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;
