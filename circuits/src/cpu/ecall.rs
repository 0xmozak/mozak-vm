use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note that the only system call we support now is 'halt', ie ecall with x17 =
    // 93. Everything else is invalid.
    // Check: `ecall` happened when x17 register evaluated to 93.
    yield_constr.constraint(lv.inst.ops.ecall * (lv.regs[17] - P::Scalar::from_canonical_u8(93)));
    // Check: `ecall` happened and we `halt`.
    yield_constr.constraint(lv.inst.ops.ecall - lv.halt);

    // Check: after halting means we do not bump the pc.
    yield_constr.constraint_transition(lv.halt * (nv.inst.pc - lv.inst.pc));
}

// We are already testing ecall with our coda of every `simple_test_code`.
