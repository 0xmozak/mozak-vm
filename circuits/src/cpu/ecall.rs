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
    // ECALL is used for HALT, IO_READ_PRIVATE/IO_READ_PUBLIC or POSEIDON2 system
    // call. So when instruction is ECALL, only one of them will be one.
    cb.always(lv.is_poseidon2.is_binary());
    cb.always(lv.is_halt.is_binary());
    cb.always(lv.is_io_store_private.is_binary());
    cb.always(lv.is_io_store_public.is_binary());
    cb.always(lv.is_call_tape.is_binary());
    cb.always(lv.is_event_tape.is_binary());
    cb.always(lv.is_events_commitment_tape.is_binary());
    cb.always(lv.is_cast_list_commitment_tape.is_binary());
    cb.always(lv.is_self_prog_id_tape.is_binary());
    cb.always(
        lv.inst.ops.ecall
            - (lv.is_halt
                + lv.is_io_store_private
                + lv.is_io_store_public
                + lv.is_call_tape
                + lv.is_event_tape
                + lv.is_events_commitment_tape
                + lv.is_cast_list_commitment_tape
                + lv.is_self_prog_id_tape
                + lv.is_poseidon2),
    );
    halt_constraints(lv, nv, cb);
    io_constraints(lv, cb);
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
    cb.transition(lv.is_halt * (lv.inst.ops.ecall + nv.is_running - 1));
    cb.always(lv.is_halt * (lv.op1_value - i64::from(ecall::HALT)));

    // We also need to make sure that the program counter is not changed by the
    // 'halt' system call.
    // Enable only for halt !!!
    cb.transition(lv.is_halt * (lv.inst.ops.ecall * (nv.inst.pc - lv.inst.pc)));

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

pub(crate) fn io_constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.always(lv.is_io_store_private * (lv.op1_value - i64::from(ecall::IO_READ_PRIVATE)));
    cb.always(lv.is_io_store_public * (lv.op1_value - i64::from(ecall::IO_READ_PUBLIC)));
    cb.always(lv.is_call_tape * (lv.op1_value - i64::from(ecall::IO_READ_CALL_TAPE)));
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
