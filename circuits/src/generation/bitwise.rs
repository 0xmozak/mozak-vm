use mozak_vm::instruction::Op;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::bitwise::columns as bitwise_cols;
use crate::lookup::permute_cols;
use crate::utils::{from_, limbs_from_u32};

#[must_use]
fn filter_xor_trace(step_rows: &[Row]) -> Vec<Row> {
    step_rows
        .into_iter()
        .filter(|row| {
            let inst = row.state.current_instruction();
            inst.op == Op::XOR
        })
        .cloned()
        .collect()
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_bitwise_trace<F: RichField>(
    step_rows: &[Row],
) -> [Vec<F>; bitwise_cols::NUM_BITWISE_COL] {
    let filtered_step_rows = filter_xor_trace(step_rows);
    let trace_len = filtered_step_rows.len();
    let max_trace_len = trace_len.max(bitwise_cols::BITWISE_U8_SIZE);
    let ext_trace_len = max_trace_len.next_power_of_two();
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; ext_trace_len]; bitwise_cols::NUM_BITWISE_COL];
    for (i, Row { state, .. }) in filtered_step_rows.iter().enumerate() {
        let inst = state.current_instruction();
        let opd1_value = state.get_register_value(inst.args.rs1);
        let opd2_value = state.get_register_value(inst.args.rs2);
        let opd2_imm_value = opd2_value.wrapping_add(inst.args.imm);
        let dst_value = state.get_register_value(inst.args.rd);

        trace[bitwise_cols::OP1][i] = from_(opd1_value);
        trace[bitwise_cols::OP2][i] = from_(opd2_imm_value);
        trace[bitwise_cols::RES][i] = from_(dst_value);
        let op1_limbs = limbs_from_u32(opd1_value);
        let op2_limbs = limbs_from_u32(opd2_imm_value);
        let dst_limbs = limbs_from_u32(dst_value);
        for j in 0..4 {
            trace[bitwise_cols::OP1_LIMBS.start + j][i] = from_(op1_limbs[j]);
            trace[bitwise_cols::OP2_LIMBS.start + j][i] = from_(op2_limbs[j]);
            trace[bitwise_cols::RES_LIMBS.start + j][i] = from_(dst_limbs[j]);
        }
    }

    // add FIXED bitwise table
    // 2^8 * 2^8 possible rows
    let mut index = 0;
    for op1 in 0..bitwise_cols::RANGE_CHECK_U8_SIZE {
        trace[bitwise_cols::FIX_RANGE_CHECK_U8][op1] = from_(op1 as u128);

        for op2 in 0..bitwise_cols::RANGE_CHECK_U8_SIZE {
            let res_xor = op1 ^ op2;
            trace[bitwise_cols::FIX_BITWISE_OP1][index] = from_(op1 as u128);
            trace[bitwise_cols::FIX_BITWISE_OP2][index] = from_(op2 as u128);
            trace[bitwise_cols::FIX_BITWISE_RES][index] = from_(res_xor as u128);
            index += 1;
        }
    }

    // add the permutation information
    for i in 0..4 {
        let (permuted_inputs, permuted_table) = permute_cols(
            &trace[bitwise_cols::OP1_LIMBS.start + i],
            &trace[bitwise_cols::FIX_RANGE_CHECK_U8],
        );
        trace[bitwise_cols::OP1_LIMBS_PERMUTED.start + i] = permuted_inputs;
        trace[bitwise_cols::FIX_RANGE_CHECK_U8_PERMUTED.start + i] = permuted_table;

        let (permuted_inputs, permuted_table) = permute_cols(
            &trace[bitwise_cols::OP2_LIMBS.start + i],
            &trace[bitwise_cols::FIX_RANGE_CHECK_U8],
        );
        trace[bitwise_cols::OP2_LIMBS_PERMUTED.start + i] = permuted_inputs;
        // trace[bitwise_cols::FIX_RANGE_CHECK_U8_PERMUTED.start + 4 + i] = permuted_table;

        let (permuted_inputs, permuted_table) = permute_cols(
            &trace[bitwise_cols::RES_LIMBS.start + i],
            &trace[bitwise_cols::FIX_RANGE_CHECK_U8],
        );
        trace[bitwise_cols::RES_LIMBS_PERMUTED.start + i] = permuted_inputs;
        // trace[bitwise_cols::FIX_RANGE_CHECK_U8_PERMUTED.start + 8 + i] = permuted_table;
    }
    let trace_row_vecs = trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            bitwise_cols::NUM_BITWISE_COL,
            v.len()
        )
    });
    trace_row_vecs
}
