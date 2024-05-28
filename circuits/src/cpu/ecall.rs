//! This module implements the constraints for the environment call operation
//! 'ECALL'.

use expr::Expr;
use mozak_sdk::core::ecall;

use super::columns::CpuState;
use crate::expr::ConstraintBuilder;

pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let ecalls = &lv.ecall_selectors;
    // ECALL is used for HALT, PRIVATE_TAPE/PUBLIC_TAPE or POSEIDON2 system
    // call. So when instruction is ECALL, only one of them will be one.
    for ecall in ecalls {
        cb.always(ecall.is_binary());
    }
    cb.always(lv.inst.ops.ecall - ecalls.iter().sum::<Expr<'a, P>>());
    cb.always(lv.ecall_selectors.is_halt * (lv.op1_value - i64::from(ecall::HALT)));
    storage_device_constraints(lv, cb);
    poseidon2_constraints(lv, cb);
}

pub(crate) fn storage_device_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let ecalls = &lv.ecall_selectors;
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
    cb.always(
        lv.ecall_selectors.is_self_prog_id_tape
            * (lv.op1_value - i64::from(ecall::SELF_PROG_ID_TAPE)),
    );
}

pub(crate) fn poseidon2_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.always(lv.ecall_selectors.is_poseidon2 * (lv.op1_value - i64::from(ecall::POSEIDON2)));
}

// We are already testing ecall halt with our coda of every `code::execute`.
