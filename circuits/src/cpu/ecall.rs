//! This module implements the constraints for the environment call operation
//! 'ECALL'.

use itertools::izip;
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
    // ECALL is used for HALT, IO_READ or POSEIDON2 system call.
    // So when instruciton is ECALL, only one of them will be one.
    yield_constr.constraint(lv.inst.ops.ecall - (lv.is_halt + lv.is_io_store + lv.is_poseidon2));
    halt_constraints(lv, nv, yield_constr);
    io_constraints(lv, yield_constr);
    poseidon2_constraints(lv, yield_constr);
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
    yield_constr.constraint(
        lv.is_halt * (lv.regs[REG_A0 as usize] - P::Scalar::from_canonical_u32(ecall::HALT)),
    );

    // We also need to make sure that the program counter is not changed by the
    // 'halt' system call.
    // Enable only for halt !!!
    yield_constr
        .constraint_transition(lv.is_halt * (lv.inst.ops.ecall * (nv.inst.pc - lv.inst.pc)));

    let is_halted = P::ONES - lv.is_running;
    is_binary(yield_constr, lv.is_running);

    // TODO: change this when we support segmented proving.
    // Last row must be 'halted', ie no longer is_running.
    yield_constr.constraint_last_row(lv.is_running);

    // Once we stop running, no subsequent row starts running again:
    yield_constr.constraint_transition(is_halted * (nv.is_running - lv.is_running));
    // Halted means that nothing changes anymore:
    for (&lv_entry, &nv_entry) in izip!(lv, nv) {
        yield_constr.constraint_transition(is_halted * (lv_entry - nv_entry));
    }
}

pub(crate) fn io_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.is_io_store);
    yield_constr.constraint(lv.is_io_store * (lv.inst.ops.ecall - P::ONES));
    yield_constr.constraint(
        lv.is_io_store * (lv.regs[REG_A0 as usize] - P::Scalar::from_canonical_u32(ecall::IO_READ)),
    );
}

pub(crate) fn poseidon2_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.is_poseidon2);
    yield_constr.constraint(lv.is_poseidon2 * (lv.inst.ops.ecall - P::ONES));
    yield_constr.constraint(
        lv.is_poseidon2
            * (lv.regs[REG_A0 as usize] - P::Scalar::from_canonical_u32(ecall::POSEIDON2)),
    );
}
// We are already testing ecall halt with our coda of every `simple_test_code`.
