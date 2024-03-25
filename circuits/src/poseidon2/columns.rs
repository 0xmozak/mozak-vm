use plonky2::hash::poseidon2::{ROUND_F_END, ROUND_P, WIDTH};

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
#[cfg(feature = "enable_poseidon_starks")]
use crate::linear_combination::ColumnUntyped;
#[cfg(feature = "enable_poseidon_starks")]
use crate::stark::mozak_stark::{Poseidon2Table, TableWithUntypedInput};

/// The size of the state

pub const STATE_SIZE: usize = WIDTH;

/// Poseidon2 constants
pub(crate) const ROUNDS_F: usize = ROUND_F_END;
pub(crate) const ROUNDS_P: usize = ROUND_P;

pub(crate) const STATE_SIZE_AFTER: usize = STATE_SIZE * (ROUNDS_F / 2);

columns_view_impl!(Poseidon2State);
make_col_map!(Poseidon2State);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Poseidon2State<F> {
    pub is_exe: F,
    pub input: [F; STATE_SIZE],
    pub state_after_first_full_rounds: [F; STATE_SIZE_AFTER],
    pub state0_after_partial_rounds: [F; ROUNDS_P],
    pub state_after_partial_rounds: [F; STATE_SIZE],
    pub state_after_second_full_rounds: [F; STATE_SIZE_AFTER],
    // following columns are used to reduce s_box computation degree
    pub s_box_input_qube_first_full_rounds: [F; STATE_SIZE_AFTER],
    pub s_box_input_qube_second_full_rounds: [F; STATE_SIZE_AFTER],
    pub s_box_input_qube_partial_rounds: [F; ROUNDS_P],
}

// TODO(Matthias): see https://users.rust-lang.org/t/cannot-default-slices-bigger-than-32-items/4947
impl<F: Default + Copy> Default for Poseidon2State<F> {
    fn default() -> Self {
        Self {
            is_exe: F::default(),
            input: [F::default(); STATE_SIZE],
            state_after_first_full_rounds: [F::default(); STATE_SIZE_AFTER],
            state0_after_partial_rounds: [F::default(); ROUNDS_P],
            state_after_partial_rounds: [F::default(); STATE_SIZE],
            state_after_second_full_rounds: [F::default(); STATE_SIZE_AFTER],
            s_box_input_qube_first_full_rounds: [F::default(); STATE_SIZE_AFTER],
            s_box_input_qube_second_full_rounds: [F::default(); STATE_SIZE_AFTER],
            s_box_input_qube_partial_rounds: [F::default(); ROUNDS_P],
        }
    }
}

pub const NUM_POSEIDON2_COLS: usize = Poseidon2State::<()>::NUMBER_OF_COLUMNS;

columns_view_impl!(Poseidon2StateCtl);
#[repr(C)]
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug)]
pub struct Poseidon2StateCtl<F> {
    pub input: [F; STATE_SIZE],
    pub output: [F; STATE_SIZE],
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_sponge() -> TableWithUntypedInput<Poseidon2StateCtl<ColumnUntyped>> {
    let poseidon2 = COL_MAP;
    // Extend data with outputs which is basically state after last full round.
    Poseidon2Table::new(
        Poseidon2StateCtl {
            input: poseidon2.input,
            output: poseidon2.state_after_second_full_rounds
                [STATE_SIZE * (ROUNDS_F / 2 - 1)..STATE_SIZE * (ROUNDS_F / 2)]
                .try_into()
                .unwrap(),
        },
        COL_MAP.is_exe,
    )
}
