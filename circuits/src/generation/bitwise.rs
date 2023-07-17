use itertools::Itertools;
use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

use crate::bitwise::columns as cols;
use crate::lookup::permute_cols;
use crate::utils::from_;

#[must_use]
fn filter_bitwise_trace(step_rows: &[Row]) -> Vec<&Row> {
    step_rows
        .iter()
        .filter(|row| matches!(row.state.current_instruction().op, Op::AND | Op::OR | Op::XOR))
        .collect()
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_bitwise_trace<F: RichField>(
    step_rows: &[Row],
) -> ([Vec<F>; cols::NUM_BITWISE_COL], F) {
    let filtered_step_rows = filter_bitwise_trace(step_rows);
    let trace_len = filtered_step_rows.len();
    let ext_trace_len = trace_len.max(cols::BITWISE_U8_SIZE).next_power_of_two();
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; cols::NUM_BITWISE_COL];
    for (i, Row { state, aux }) in filtered_step_rows.iter().enumerate() {
        let inst = state.current_instruction();
        let opd1_value = state.get_register_value(inst.args.rs1);
        let opd2_value = state.get_register_value(inst.args.rs2);
        let opd2_imm_value = opd2_value.wrapping_add(inst.args.imm);

        trace[cols::OP1][i] = from_(opd1_value);
        trace[cols::OP2][i] = from_(opd2_imm_value);
        trace[cols::RES][i] = from_(aux.dst_val);
        for (cols, limbs) in [
            (cols::OP1_LIMBS, opd1_value.to_le_bytes()),
            (cols::OP2_LIMBS, opd2_imm_value.to_le_bytes()),
            (cols::RES_LIMBS, aux.dst_val.to_le_bytes()),
        ] {
            for (col, limb) in cols.zip(limbs) {
                trace[col][i] = from_(limb);
            }
        }
    }

    // add FIXED bitwise table
    // 2^8 * 2^8 possible rows
    trace[cols::FIX_RANGE_CHECK_U8] = cols::RANGE_U8.map(from_).collect();
    trace[cols::FIX_RANGE_CHECK_U8].resize(ext_trace_len, F::ZERO);

    for (index, (op1, op2)) in cols::RANGE_U8.cartesian_product(cols::RANGE_U8).enumerate() {
        trace[cols::FIX_BITWISE_OP1][index] = from_(op1);
        trace[cols::FIX_BITWISE_OP2][index] = from_(op2);
        trace[cols::FIX_BITWISE_RES][index] = from_(op1 ^ op2);
    }

    let mut challenger =
        Challenger::<F, <PoseidonGoldilocksConfig as GenericConfig<2>>::Hasher>::new();
    for limb in cols::OP1_LIMBS
        .chain(cols::OP2_LIMBS)
        .chain(cols::RES_LIMBS)
    {
        challenger.observe_elements(&trace[limb]);
    }
    let beta = challenger.get_challenge();
    // TODO: Fix following issues related to possible security risks due to this
    // randomness. https://github.com/0xmozak/mozak-vm/issues/310
    // https://github.com/0xmozak/mozak-vm/issues/309

    for i in 0..trace[0].len() {
        for (((compress_limb, op1_limb), op2_limb), res_limb) in cols::COMPRESS_LIMBS
            .zip(cols::OP1_LIMBS)
            .zip(cols::OP2_LIMBS)
            .zip(cols::RES_LIMBS)
        {
            trace[compress_limb][i] =
                trace[op1_limb][i] + beta * (trace[op2_limb][i] + beta * trace[res_limb][i]);
        }

        trace[cols::FIX_COMPRESS][i] = trace[cols::FIX_BITWISE_OP1][i]
            + beta * (trace[cols::FIX_BITWISE_OP2][i] + beta * trace[cols::FIX_BITWISE_RES][i]);
    }

    // add the permutation information
    for (op_limbs_permuted, range_check_permuted, op_limbs, table_col) in [
        (
            cols::OP1_LIMBS_PERMUTED,
            cols::FIX_RANGE_CHECK_U8_PERMUTED.skip(0),
            cols::OP1_LIMBS,
            cols::FIX_RANGE_CHECK_U8,
        ),
        (
            cols::OP2_LIMBS_PERMUTED,
            cols::FIX_RANGE_CHECK_U8_PERMUTED.skip(4),
            cols::OP2_LIMBS,
            cols::FIX_RANGE_CHECK_U8,
        ),
        (
            cols::RES_LIMBS_PERMUTED,
            cols::FIX_RANGE_CHECK_U8_PERMUTED.skip(8),
            cols::RES_LIMBS,
            cols::FIX_RANGE_CHECK_U8,
        ),
        (
            cols::COMPRESS_PERMUTED,
            cols::FIX_COMPRESS_PERMUTED.skip(0),
            cols::COMPRESS_LIMBS,
            cols::FIX_COMPRESS,
        ),
    ] {
        for ((op_limb_permuted, range_check_limb_permuted), op_limb) in
            op_limbs_permuted.zip(range_check_permuted).zip(op_limbs)
        {
            (trace[op_limb_permuted], trace[range_check_limb_permuted]) =
                permute_cols(&trace[op_limb], &trace[table_col]);
        }
    }

    let trace_row_vecs = trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cols::NUM_BITWISE_COL,
            v.len()
        )
    });
    log::trace!("trace {:?}", trace_row_vecs);
    (trace_row_vecs, beta)
}
