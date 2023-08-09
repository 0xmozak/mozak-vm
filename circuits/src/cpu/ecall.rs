use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // At the moment, the only system call we support is 'halt', ie ecall with x17 =
    // 93. Everything else is invalid.
    yield_constr.constraint(lv.inst.ops.ecall * (lv.regs[17] - P::Scalar::from_canonical_u8(93)));
    // Thus we can equate ecall with halt in the next row.
    yield_constr.constraint_transition(lv.inst.ops.ecall - nv.halt);
}

// We are already testing ecall with our coda of every `simple_test_code`.
