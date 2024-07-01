use core::fmt::Debug;
use std::fmt::Display;

use super::columns::ProgramMult;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct ProgramMultConstraints {}

const COLUMNS: usize = ProgramMult::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for ProgramMultConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = ProgramMult<E>;
}

impl Display for ProgramMultConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[allow(clippy::module_name_repetitions)]
pub type ProgramMultStark<F, const D: usize> =
    StarkFrom<F, ProgramMultConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;
