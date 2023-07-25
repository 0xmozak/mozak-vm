use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;
use starky::vars::StarkEvaluationVars;

use super::columns::MAP;
use crate::lookup::eval_lookups;

pub fn constraints_on_shamt<
    F: Field,
    P: PackedField<Scalar = F>,
    const COLS: usize,
    const PUBLIC_INPUTS: usize,
>(
    vars: StarkEvaluationVars<F, P, COLS, PUBLIC_INPUTS>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let lv = vars.local_values;
    let nv = vars.next_values;
    yield_constr.constraint_first_row(lv[MAP.shift_amount.fixed_shamt]);
    yield_constr.constraint_transition(
        (nv[MAP.shift_amount.fixed_shamt] - lv[MAP.shift_amount.fixed_shamt] - P::ONES)
            * (nv[MAP.shift_amount.fixed_shamt] - lv[MAP.shift_amount.fixed_shamt]),
    );
    yield_constr
        .constraint_last_row(lv[MAP.shift_amount.fixed_shamt] - P::Scalar::from_canonical_u8(31));
    eval_lookups(
        vars,
        yield_constr,
        MAP.shift_amount.powers_of_2_in_permuted,
        MAP.shift_amount.fixed_shamt_permuted,
    );
}

pub fn constraints_on_power_of_2_shamt<
    F: Field,
    P: PackedField<Scalar = F>,
    const COLS: usize,
    const PUBLIC_INPUTS: usize,
>(
    vars: StarkEvaluationVars<F, P, COLS, PUBLIC_INPUTS>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let lv = vars.local_values;
    let nv = vars.next_values;
    let diff = nv[MAP.shift_amount.fixed_shamt] - lv[MAP.shift_amount.fixed_shamt];
    yield_constr.constraint_first_row(lv[MAP.shift_amount.fixed_power_of_2_shamt] - P::ONES);
    yield_constr.constraint_transition(
        nv[MAP.shift_amount.fixed_power_of_2_shamt]
            - (P::ONES + diff) * lv[MAP.shift_amount.fixed_power_of_2_shamt],
    );
    yield_constr.constraint_last_row(
        lv[MAP.shift_amount.fixed_power_of_2_shamt] - P::Scalar::from_canonical_u32(1 << 31),
    );
    eval_lookups(
        vars,
        yield_constr,
        MAP.shift_amount.powers_of_2_out_permuted,
        MAP.shift_amount.fixed_power_of_2_shamt_permuted,
    );
}
