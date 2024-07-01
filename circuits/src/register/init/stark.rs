use core::fmt::Debug;
use std::fmt::Display;

use super::columns::RegisterInit;
use crate::columns_view::NumberOfColumns;
use crate::expr::{GenerateConstraints, StarkFrom};
use crate::unstark::NoColumns;

#[derive(Default, Clone, Copy, Debug)]
pub struct RegisterInitConstraints {}

const COLUMNS: usize = RegisterInit::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for RegisterInitConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = RegisterInit<E>;
}

impl Display for RegisterInitConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

/// For sanity check, we can constrain the register address column to be in
/// a running sum from 0..=31, but since this fixed table is known to
/// both prover and verifier, we do not need to do so here.
#[allow(clippy::module_name_repetitions)]
pub type RegisterInitStark<F, const D: usize> =
    StarkFrom<F, RegisterInitConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;
