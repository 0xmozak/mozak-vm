use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;

/// The size of the state
pub const STATE_SIZE: usize = 12;
pub(crate) const SBOX_DEGREE: usize = 7;

/// Poseidon2 constants
pub(crate) const ROUNDS_F: usize = 8;
pub(crate) const ROUNDS_P: usize = 22;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Poseidon2State<F> {
    pub is_exe: F,
    pub input: [F; STATE_SIZE],
    pub state_after_first_full_rounds: [F; STATE_SIZE * (ROUNDS_F / 2)],
    pub state0_after_partial_rounds: [F; ROUNDS_P],
    pub state_after_partial_rounds: [F; STATE_SIZE],
    pub state_after_second_full_rounds: [F; STATE_SIZE * (ROUNDS_F / 2)],
}

impl<F: Default + Copy> Default for Poseidon2State<F> {
    fn default() -> Self {
        Self {
            is_exe: F::default(),
            input: [F::default(); STATE_SIZE],
            state_after_first_full_rounds: [F::default(); STATE_SIZE * (ROUNDS_F / 2)],
            state0_after_partial_rounds: [F::default(); ROUNDS_P],
            state_after_partial_rounds: [F::default(); STATE_SIZE],
            state_after_second_full_rounds: [F::default(); STATE_SIZE * (ROUNDS_F / 2)],
        }
    }
}

columns_view_impl!(Poseidon2State);
make_col_map!(Poseidon2State);

pub const NUM_POSEIDON2_COLS: usize = Poseidon2State::<()>::NUMBER_OF_COLUMNS;

pub fn data_for_sponge<F: Field>() -> Vec<Column<F>> {
    let poseidon2 = MAP.map(Column::from);
    let mut data = poseidon2.input.to_vec();
    // exten data with outputs which is basically state after last full round.
    data.extend(
        poseidon2.state_after_second_full_rounds
            [STATE_SIZE * (ROUNDS_F / 2 - 1)..STATE_SIZE * (ROUNDS_F / 2)]
            .to_vec(),
    );
    data
}

pub fn filter_for_sponge<F: Field>() -> Column<F> {
    let poseidon2 = MAP.map(Column::from);
    poseidon2.is_exe
}
