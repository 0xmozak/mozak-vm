use super::columns::RegisterInit;
use crate::columns_view::NumberOfColumns;
use crate::unstark::Unstark;

/// For sanity check, we can constrain the register address column to be in
/// a running sum from 0..=31, but since this fixed table is known to
/// both prover and verifier, we do not need to do so here.
#[allow(clippy::module_name_repetitions)]
pub type RegisterInitStark<F, const D: usize> =
    Unstark<F, D, RegisterInit<F>, { RegisterInit::<()>::NUMBER_OF_COLUMNS }>;
