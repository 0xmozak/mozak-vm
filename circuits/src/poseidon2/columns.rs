use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
use crate::stark::mozak_stark::{Poseidon2Table, Table};

/// The size of the state
pub const STATE_SIZE: usize = 12;

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
    // following columns are used to reduce s_box computation degree
    pub s_box_input_qube_first_full_rounds: [F; STATE_SIZE * (ROUNDS_F / 2)],
    pub s_box_input_qube_second_full_rounds: [F; STATE_SIZE * (ROUNDS_F / 2)],
    pub s_box_input_qube_partial_rounds: [F; ROUNDS_P],
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
            s_box_input_qube_first_full_rounds: [F::default(); STATE_SIZE * (ROUNDS_F / 2)],
            s_box_input_qube_second_full_rounds: [F::default(); STATE_SIZE * (ROUNDS_F / 2)],
            s_box_input_qube_partial_rounds: [F::default(); ROUNDS_P],
        }
    }
}

columns_view_impl!(Poseidon2State);
make_col_map!(Poseidon2State);

pub const NUM_POSEIDON2_COLS: usize = Poseidon2State::<()>::NUMBER_OF_COLUMNS;

pub fn lookup_for_sponge() -> Table {
    let poseidon2 = col_map().map(Column::from);
    let mut data = poseidon2.input.to_vec();
    // Extend data with outputs which is basically state after last full round.
    data.extend(
        poseidon2.state_after_second_full_rounds
            [STATE_SIZE * (ROUNDS_F / 2 - 1)..STATE_SIZE * (ROUNDS_F / 2)]
            .to_vec(),
    );
    Poseidon2Table::new(data, poseidon2.is_exe)
}
