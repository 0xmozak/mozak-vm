use itertools::Itertools;
use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

use crate::bitwise::columns as bitwise_cols;
use crate::lookup::permute_cols;
use crate::utils::from_;

#[must_use]
fn filter_bitwise_trace(step_rows: &[Row]) -> Vec<Row> {
    step_rows
        .iter()
        .filter(|row| {
            let inst = row.state.current_instruction();
            inst.op == Op::AND
        })
        .cloned()
        .collect()
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_bitwise_trace<F: RichField>(
    step_rows: &[Row],
) -> ([Vec<F>; bitwise_cols::NUM_BITWISE_COL], F) {
    let filtered_step_rows = filter_bitwise_trace(step_rows);
    let trace_len = filtered_step_rows.len();
    let max_trace_len = trace_len.max(bitwise_cols::BITWISE_U8_SIZE);
    let ext_trace_len = max_trace_len.next_power_of_two();
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; bitwise_cols::NUM_BITWISE_COL];
    for (i, Row { state, aux }) in filtered_step_rows.iter().enumerate() {
        let inst = state.current_instruction();
        let opd1_value = state.get_register_value(inst.args.rs1);
        let opd2_value = state.get_register_value(inst.args.rs2);
        let opd2_imm_value = opd2_value.wrapping_add(inst.args.imm);

        trace[bitwise_cols::OP1][i] = from_(opd1_value);
        trace[bitwise_cols::OP2][i] = from_(opd2_imm_value);
        trace[bitwise_cols::RES][i] = from_(aux.dst_val);
        for (cols, limbs) in [
            (bitwise_cols::OP1_LIMBS, opd1_value.to_le_bytes()),
            (bitwise_cols::OP2_LIMBS, opd2_imm_value.to_le_bytes()),
            (bitwise_cols::RES_LIMBS, aux.dst_val.to_le_bytes()),
        ] {
            for (c, l) in cols.zip(limbs) {
                trace[c][i] = from_(l);
            }
        }
    }

    // add FIXED bitwise table
    // 2^8 * 2^8 possible rows
    trace[bitwise_cols::FIX_RANGE_CHECK_U8] = (0..bitwise_cols::RANGE_CHECK_U8_SIZE)
        .map(|op1| from_(op1 as u128))
        .collect();
    trace[bitwise_cols::FIX_RANGE_CHECK_U8].resize(ext_trace_len, F::ZERO);
    for (index, (op1, op2)) in (0..bitwise_cols::RANGE_CHECK_U8_SIZE)
        .cartesian_product(0..bitwise_cols::RANGE_CHECK_U8_SIZE)
        .enumerate()
    {
        let res_and = op1 & op2;
        trace[bitwise_cols::FIX_BITWISE_OP1][index] = from_(op1 as u128);
        trace[bitwise_cols::FIX_BITWISE_OP2][index] = from_(op2 as u128);
        trace[bitwise_cols::FIX_BITWISE_RES][index] = from_(res_and as u128);
    }

    let mut challenger =
        Challenger::<F, <PoseidonGoldilocksConfig as GenericConfig<2>>::Hasher>::new();
    for limb in bitwise_cols::OP1_LIMBS
        .chain(bitwise_cols::OP2_LIMBS)
        .chain(bitwise_cols::RES_LIMBS)
    {
        challenger.observe_elements(&trace[limb]);
    }
    let beta = challenger.get_challenge();

    for i in 0..trace[0].len() {
        // (bitwise_cols::OP1_LIMBS, opd1_value.to_le_bytes()),
        for (((a, b), c), d) in bitwise_cols::COMPRESS_LIMBS
            .zip(bitwise_cols::OP1_LIMBS)
            .zip(bitwise_cols::OP2_LIMBS)
            .zip(bitwise_cols::RES_LIMBS)
        {
            trace[a][i] = trace[b][i] + beta * (trace[c][i] + beta * trace[d][i]);
        }

        trace[bitwise_cols::FIX_COMPRESS][i] = trace[bitwise_cols::FIX_BITWISE_OP1][i]
            + trace[bitwise_cols::FIX_BITWISE_OP2][i] * beta
            + trace[bitwise_cols::FIX_BITWISE_RES][i] * beta * beta;
    }

    // add the permutation information
    for (op_limbs_permuted, range_check_permuted, op_limbs) in [
        (
            bitwise_cols::OP1_LIMBS_PERMUTED,
            bitwise_cols::FIX_RANGE_CHECK_U8_PERMUTED.skip(0),
            bitwise_cols::OP1_LIMBS,
        ),
        (
            bitwise_cols::OP2_LIMBS_PERMUTED,
            bitwise_cols::FIX_RANGE_CHECK_U8_PERMUTED.skip(4),
            bitwise_cols::OP2_LIMBS,
        ),
        (
            bitwise_cols::RES_LIMBS_PERMUTED,
            bitwise_cols::FIX_RANGE_CHECK_U8_PERMUTED.skip(8),
            bitwise_cols::OP2_LIMBS,
        ),
    ] {
        for ((op_limb_permuted, range_check_limb_permuted), op_limb) in
            op_limbs_permuted.zip(range_check_permuted).zip(op_limbs)
        {
            let (permuted_inputs, permuted_table) =
                permute_cols(&trace[op_limb], &trace[bitwise_cols::FIX_RANGE_CHECK_U8]);
            trace[op_limb_permuted] = permuted_inputs;
            trace[range_check_limb_permuted] = permuted_table;
        }
    }
    let trace_row_vecs = trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            bitwise_cols::NUM_BITWISE_COL,
            v.len()
        )
    });
    log::trace!("trace {:?}", trace_row_vecs);
    (trace_row_vecs, beta)
}
