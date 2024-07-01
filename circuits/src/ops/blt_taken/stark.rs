use core::fmt::Debug;
use std::fmt::Display;

use super::columns::BltTaken;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct BltTakenConstraints {}

const COLUMNS: usize = BltTaken::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for BltTakenConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = BltTaken<E>;
}

impl Display for BltTakenConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[allow(clippy::module_name_repetitions)]
pub type BltTakenStark<F, const D: usize> =
    StarkFrom<F, BltTakenConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;
