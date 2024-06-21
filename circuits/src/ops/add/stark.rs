use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::Add;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct AddConstraints {}

pub type AddStark<F, const D: usize> =
    StarkFrom<F, AddConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

impl<F, const D: usize> HasNamedColumns for AddStark<F, D> {
    type Columns = Add<F>;
}

const COLUMNS: usize = Add::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for AddConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = Add<E>;

    fn generate_constraints<'a, T: Copy + Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let mut constraints = ConstraintBuilder::default();

        let added = lv.op1_value + lv.op2_value + lv.inst.imm_value;
        let wrapped = added - (1 << 32);

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        constraints.always((lv.dst_value - added) * (lv.dst_value - wrapped));

        constraints
    }
}
