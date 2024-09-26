use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::CpuSkeleton;
use crate::columns_view::NumberOfColumns;
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::stark::mozak_stark::PublicInputs;

// TODO: fix StarkNameDisplay?
#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuSkeletonConstraints {}

#[allow(clippy::module_name_repetitions)]
pub type CpuSkeletonStark<F, const D: usize> =
    StarkFrom<F, CpuSkeletonConstraints, { D }, COLUMNS, PUBLIC_INPUTS>;

const COLUMNS: usize = CpuSkeleton::<()>::NUMBER_OF_COLUMNS;
// Public inputs: [PC of the first row]
const PUBLIC_INPUTS: usize = PublicInputs::<()>::NUMBER_OF_COLUMNS;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for CpuSkeletonConstraints {
    type PublicInputs<E: Debug> = PublicInputs<E>;
    type View<E: Debug> = CpuSkeleton<E>;

    fn generate_constraints<'a, T: Debug + Copy>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let nv = vars.next_values;
        let public_inputs = vars.public_inputs;
        let mut constraints = ConstraintBuilder::default();

        constraints.first_row(lv.pc - public_inputs.entry_point);
        // Clock starts at 2. This is to differentiate
        // execution clocks (2 and above) from
        // clk values `0` and `1` which are reserved for
        // elf initialisation and zero initialisation respectively.
        constraints.first_row(lv.clk - 2);

        let clock_diff = nv.clk - lv.clk;
        constraints.transition(clock_diff.is_binary());

        // clock only counts up when we are still running.
        constraints.transition(clock_diff - lv.is_running);

        // We start in running state.
        constraints.first_row(lv.is_running - 1);

        // We may transition to a non-running state.
        constraints.transition(nv.is_running * (nv.is_running - lv.is_running));

        // We end in a non-running state.
        constraints.last_row(lv.is_running);

        // NOTE: in our old CPU table we had constraints that made sure nothing
        // changes anymore, once we are halted. We don't need those
        // anymore: the only thing that can change are memory or registers.  And
        // our CTLs make sure, that after we are halted, no more memory
        // or register changes are allowed.
        constraints
    }
}
