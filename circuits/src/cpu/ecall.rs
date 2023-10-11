//! This module implements the constraints for the environment call operation
//! 'ECALL'.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: this needs to change, when we add support for more system calls.
    // At the moment, the only system call we support is 'halt', ie ecall with x10 =
    // 0. Everything else is invalid.
    yield_constr.constraint(lv.inst.ops.ecall * (lv.reg_10 - P::Scalar::from_canonical_u8(0)));
    // Thus we can equate ecall with halt in the next row.
    // Crucially, this prevents a malicious prover from just halting the program
    // anywhere else.
    yield_constr.constraint_transition(lv.inst.ops.ecall + nv.is_running - P::ONES);

    // We also need to make sure that the program counter is not changed by the
    // 'halt' system call.
    yield_constr.constraint_transition(lv.inst.ops.ecall * (nv.inst.pc - lv.inst.pc));
}

// We are already testing ecall with our coda of every `simple_test_code`.
