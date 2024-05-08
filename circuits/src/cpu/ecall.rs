//! This module implements the constraints for the environment call operation
//! 'ECALL'.

use expr::Expr;
use itertools::izip;
use mozak_sdk::core::ecall;

use super::columns::CpuState;
use crate::expr::ConstraintBuilder;

pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    nv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let ecalls = &lv.ecall_selectors;
    // ECALL is used for HALT, PRIVATE_TAPE/PUBLIC_TAPE or POSEIDON2 system
    // call. So when instruction is ECALL, only one of them will be one.
    for ecall in ecalls {
        cb.always(ecall.is_binary());
    }
    cb.always(lv.inst.ops.ecall - ecalls.iter().sum::<Expr<'a, P>>());
    halt_constraints(lv, nv, cb);
    storage_device_constraints(lv, cb);
    poseidon2_constraints(lv, cb);
}

pub(crate) fn halt_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    nv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    // Thus we can equate ecall with halt in the next row.
    // Crucially, this prevents a malicious prover from just halting the program
    // anywhere else.
    // Enable only for halt !!!
    cb.transition(lv.ecall_selectors.is_halt * (lv.inst.ops.ecall + nv.is_running - 1));
    cb.always(lv.ecall_selectors.is_halt * (lv.op1_value - i64::from(ecall::HALT)));

    // We also need to make sure that the program counter is not changed by the
    // 'halt' system call.
    // Enable only for halt !!!
    cb.transition(lv.ecall_selectors.is_halt * (lv.inst.ops.ecall * (nv.inst.pc - lv.inst.pc)));

    let is_halted = 1 - lv.is_running;
    cb.always(lv.is_running.is_binary());

    // TODO: change this when we support segmented proving.
    // Last row must be 'halted', ie no longer is_running.
    cb.last_row(lv.is_running);

    // Once we stop running, no subsequent row starts running again:
    cb.transition(is_halted * (nv.is_running - lv.is_running));
    // Halted means that nothing changes anymore:
    for (&lv_entry, &nv_entry) in izip!(lv, nv) {
        cb.transition(is_halted * (lv_entry - nv_entry));
    }
}

pub(crate) fn storage_device_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let ecalls = lv.ecall_selectors;
    cb.always(ecalls.is_private_tape * (lv.op1_value - i64::from(ecall::PRIVATE_TAPE)));
    cb.always(ecalls.is_public_tape * (lv.op1_value - i64::from(ecall::PUBLIC_TAPE)));
    cb.always(ecalls.is_call_tape * (lv.op1_value - i64::from(ecall::CALL_TAPE)));
    cb.always(ecalls.is_event_tape * (lv.op1_value - i64::from(ecall::EVENT_TAPE)));
    cb.always(
        ecalls.is_events_commitment_tape
            * (lv.op1_value - i64::from(ecall::EVENTS_COMMITMENT_TAPE)),
    );
    cb.always(
        ecalls.is_cast_list_commitment_tape
            * (lv.op1_value - i64::from(ecall::CAST_LIST_COMMITMENT_TAPE)),
    );
}

pub(crate) fn poseidon2_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.always(lv.ecall_selectors.is_poseidon2 * (lv.op1_value - i64::from(ecall::POSEIDON2)));
}

// We are already testing ecall halt with our coda of every `code::execute`.
