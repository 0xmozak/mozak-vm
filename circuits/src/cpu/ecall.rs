//! This module implements the constraints for the environment call operation
//! 'ECALL'.

use itertools::izip;
use mozak_system::system::ecall;
use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::columns::CpuState;
use crate::cpu::stark::add_extension_vec;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // ECALL is used for HALT, IO_READ_PRIVATE/IO_READ_PUBLIC or POSEIDON2 system
    // call. So when instruction is ECALL, only one of them will be one.
    is_binary(yield_constr, lv.is_poseidon2);
    is_binary(yield_constr, lv.is_halt);
    is_binary(yield_constr, lv.is_io_store_private);
    is_binary(yield_constr, lv.is_io_store_public);
    yield_constr.constraint(
        lv.inst.ops.ecall
            - (lv.is_halt + lv.is_io_store_private + lv.is_io_store_public + lv.is_poseidon2),
    );
    halt_constraints(lv, nv, yield_constr);
}

pub(crate) fn halt_constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
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

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    nv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    is_binary_ext_circuit(builder, lv.is_poseidon2, yield_constr);
    is_binary_ext_circuit(builder, lv.is_halt, yield_constr);
    is_binary_ext_circuit(builder, lv.is_io_store_private, yield_constr);
    is_binary_ext_circuit(builder, lv.is_io_store_public, yield_constr);

    let is_ecall_ops = add_extension_vec(builder, vec![
        lv.is_halt,
        lv.is_io_store_private,
        lv.is_io_store_public,
        lv.is_poseidon2,
    ]);
    let ecall_constraint = builder.sub_extension(lv.inst.ops.ecall, is_ecall_ops);
    yield_constr.constraint(builder, ecall_constraint);

    halt_constraints_circuit(builder, lv, nv, yield_constr);
}

pub(crate) fn halt_constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    nv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let one = builder.one_extension();
    let halt_ecall_plus_running = builder.add_extension(lv.inst.ops.ecall, nv.is_running);
    let halt_ecall_plus_running_sub_one = builder.sub_extension(halt_ecall_plus_running, one);
    let constraint1 = builder.mul_extension(lv.is_halt, halt_ecall_plus_running_sub_one);
    yield_constr.constraint_transition(builder, constraint1);

    let halt_value = builder.constant_extension(F::Extension::from_canonical_u32(ecall::HALT));

    let nv_pc_sub_lv_pc = builder.sub_extension(nv.inst.pc, lv.inst.pc);
    let ecall_mul_nv_pc_sub_lv_pc = builder.mul_extension(lv.inst.ops.ecall, nv_pc_sub_lv_pc);
    let pc_constraint = builder.mul_extension(lv.is_halt, ecall_mul_nv_pc_sub_lv_pc);
    yield_constr.constraint_transition(builder, pc_constraint);

    let is_halted = builder.sub_extension(one, lv.is_running);
    is_binary_ext_circuit(builder, lv.is_running, yield_constr);
    yield_constr.constraint_last_row(builder, lv.is_running);

    let nv_is_running_sub_lv_is_running = builder.sub_extension(nv.is_running, lv.is_running);
    let transition_constraint = builder.mul_extension(is_halted, nv_is_running_sub_lv_is_running);
    yield_constr.constraint_transition(builder, transition_constraint);

    for (index, &lv_entry) in lv.iter().enumerate() {
        let nv_entry = nv[index];
        let lv_nv_entry_sub = builder.sub_extension(lv_entry, nv_entry);
        let transition_constraint = builder.mul_extension(is_halted, lv_nv_entry_sub);
        yield_constr.constraint_transition(builder, transition_constraint);
    }
}
