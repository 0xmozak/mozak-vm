//! This module implements the constraints for the environment call operation
//! 'ECALL'.

use mozak_runner::system::ecall;
use mozak_runner::system::reg_abi::REG_A0;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;
use crate::stark::utils::is_binary;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: this needs to change, when we add support for more system calls.
    // At the moment, the only system call we support is 'halt' or io_read, ie ecall
    // with x10 = ecall::HALT or x10 = ecall::IO_READ . Everything else is
    // invalid.
    yield_constr.constraint(
        lv.inst.ops.ecall
            * (lv.regs[REG_A0 as usize]
                - P::Scalar::from_canonical_u8(u8::try_from(ecall::HALT).unwrap()))
            * (lv.regs[REG_A0 as usize]
                - P::Scalar::from_canonical_u8(u8::try_from(ecall::IO_READ).unwrap())),
    );
    halt_constraints(lv, nv, yield_constr);
    io_constraints(lv, yield_constr);
}

pub(crate) fn halt_constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.is_halt);
    // Thus we can equate ecall with halt in the next row.
    // Crucially, this prevents a malicious prover from just halting the program
    // anywhere else.
    // Enable only for halt !!!
    yield_constr.constraint_transition(lv.is_halt * (lv.inst.ops.ecall + nv.is_running - P::ONES));

    // We also need to make sure that the program counter is not changed by the
    // 'halt' system call.
    // Enable only for halt !!!
    yield_constr
        .constraint_transition(lv.is_halt * (lv.inst.ops.ecall * (nv.inst.pc - lv.inst.pc)));
}

pub(crate) fn io_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.is_io_load);
    is_binary(yield_constr, lv.is_io_store);
    is_binary(yield_constr, lv.is_io_store + lv.is_io_load);
}
// We are already testing ecall with our coda of every `simple_test_code`.
