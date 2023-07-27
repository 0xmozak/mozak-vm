use std::borrow::Borrow;
use std::marker::PhantomData;

use itertools::Itertools;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ProgramColumnsView<T: Copy> {
    pub program_is_inst: T,
    pub program_pc: T,
}
columns_view_impl!(ProgramColumnsView);
make_col_map!(ProgramColumnsView);

// Total number of columns.
pub const NUM_PROGRAM_COLS: usize = ProgramColumnsView::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn data_for_ctl<F: Field>() -> Vec<Column<F>> {
    Column::singles([MAP.program_pc]).collect_vec()
}
#[must_use]
pub fn filter_for_ctl<F: Field>() -> Column<F> { Column::single(MAP.program_is_inst) }

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct ProgramStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for ProgramStark<F, D> {
    const COLUMNS: usize = ProgramColumnsView::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &ProgramColumnsView<P> = vars.local_values.borrow();
        yield_constr.constraint(lv.program_is_inst * (lv.program_is_inst - P::ONES));
    }

    #[no_coverage]
    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }

    fn constraint_degree(&self) -> usize { 2 }
}
