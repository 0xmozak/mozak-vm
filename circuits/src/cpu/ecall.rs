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
    // ECALL is used for HALT, PRIVATE_TAPE/PUBLIC_TAPE or POSEIDON2 system
    // call. So when instruction is ECALL, only one of them will be one.
    cb.always(lv.is_poseidon2.is_binary());
    cb.always(lv.is_halt.is_binary());
    cb.always(lv.is_private_tape.is_binary());
    cb.always(lv.is_public_tape.is_binary());
    cb.always(lv.is_call_tape.is_binary());
    cb.always(lv.is_event_tape.is_binary());
    cb.always(lv.is_events_commitment_tape.is_binary());
    cb.always(lv.is_cast_list_commitment_tape.is_binary());
    cb.always(lv.is_self_prog_id_tape.is_binary());
    cb.always(
        lv.inst.ops.ecall
            - (lv.is_halt
                + lv.is_private_tape
                + lv.is_public_tape
                + lv.is_call_tape
                + lv.is_event_tape
                + lv.is_events_commitment_tape
                + lv.is_cast_list_commitment_tape
                + lv.is_self_prog_id_tape
                + lv.is_poseidon2),
    );
    cb.always(lv.is_halt * (lv.op1_value - i64::from(ecall::HALT)));
    storage_device_constraints(lv, cb);
    poseidon2_constraints(lv, cb);
}

pub(crate) fn storage_device_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.always(lv.is_private_tape * (lv.op1_value - i64::from(ecall::PRIVATE_TAPE)));
    cb.always(lv.is_public_tape * (lv.op1_value - i64::from(ecall::PUBLIC_TAPE)));
    cb.always(lv.is_call_tape * (lv.op1_value - i64::from(ecall::CALL_TAPE)));
    cb.always(lv.is_event_tape * (lv.op1_value - i64::from(ecall::EVENT_TAPE)));
    cb.always(
        lv.is_events_commitment_tape * (lv.op1_value - i64::from(ecall::EVENTS_COMMITMENT_TAPE)),
    );
    cb.always(
        lv.is_cast_list_commitment_tape
            * (lv.op1_value - i64::from(ecall::CAST_LIST_COMMITMENT_TAPE)),
    );
    cb.always(lv.is_self_prog_id_tape * (lv.op1_value - i64::from(ecall::SELF_PROG_ID_TAPE)));
}

pub(crate) fn poseidon2_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.always(lv.is_poseidon2 * (lv.op1_value - i64::from(ecall::POSEIDON2)));
}

// We are already testing ecall halt with our coda of every `code::execute`.
