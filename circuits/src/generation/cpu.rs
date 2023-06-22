use mozak_vm::instruction::Op;
use mozak_vm::state::Aux;
use mozak_vm::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns as cpu_cols;
use crate::utils::{from_, pad_trace};

#[allow(clippy::missing_panics_doc)]
pub fn generate_cpu_trace<F: RichField>(step_rows: &[Row]) -> [Vec<F>; cpu_cols::NUM_CPU_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![F::ZERO; step_rows.len()]; cpu_cols::NUM_CPU_COLS];

    for (
        i,
        Row {
            state: s,
            aux: Aux {
                dst_val, will_halt, ..
            },
        },
    ) in step_rows.iter().enumerate()
    {
        trace[cpu_cols::COL_CLK][i] = from_(s.clk);
        trace[cpu_cols::COL_PC][i] = from_(s.get_pc());

        let inst = s.current_instruction();

        trace[cpu_cols::COL_RS1][i] = from_(inst.data.rs1);
        trace[cpu_cols::COL_RS2][i] = from_(inst.data.rs2);
        trace[cpu_cols::COL_RD][i] = from_(inst.data.rd);
        trace[cpu_cols::COL_OP1_VALUE][i] = from_(s.get_register_value(inst.data.rs1));
        trace[cpu_cols::COL_OP2_VALUE][i] = from_(s.get_register_value(inst.data.rs2));
        // NOTE: Updated value of DST register is next step.
        trace[cpu_cols::COL_DST_VALUE][i] = from_(*dst_val);
        trace[cpu_cols::COL_IMM_VALUE][i] = from_(inst.data.imm);
        trace[cpu_cols::COL_S_HALT][i] = from_(u32::from(*will_halt));
        for j in 0..32 {
            trace[cpu_cols::COL_START_REG + j as usize][i] = from_(s.get_register_value(j));
        }

        match inst.op {
            Op::ADD => trace[cpu_cols::COL_S_ADD][i] = F::ONE,
            Op::BEQ => trace[cpu_cols::COL_S_BEQ][i] = F::ONE,
            Op::ECALL => trace[cpu_cols::COL_S_ECALL][i] = F::ONE,
            #[tarpaulin::skip]
            _ => {}
        }
    }

    // For expanded trace from `trace_len` to `trace_len's power of two`,
    // we use last row `HALT` to pad them.
    let trace = pad_trace(trace, Some(cpu_cols::COL_CLK));

    log::trace!("trace {:?}", trace);
    #[tarpaulin::skip]
    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            cpu_cols::NUM_CPU_COLS,
            v.len()
        )
    })
}

#[cfg(test)]
mod test {
    use plonky2::field::{types::{Field64, Field}, goldilocks_field::GoldilocksField};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_signed(a in any::<i32>(), b in any::<i32>()) {
            let abs_diff = a.abs_diff(b);
            let a_field :GoldilocksField = GoldilocksField::from_noncanonical_i64(a as i64);
            let b_field : GoldilocksField = GoldilocksField::from_noncanonical_i64(b as i64);
            let abs_field: GoldilocksField = GoldilocksField::from_noncanonical_i64(abs_diff as i64);
            let cond  = a_field - b_field - abs_field;
            if a > b {
                assert_eq!(cond, GoldilocksField::from_noncanonical_i64(0_i64));
            } else {
                assert_ne!(cond, GoldilocksField::from_noncanonical_i64(0_i64));
                let cond_inv = cond.try_inverse().expect("can't inverse");
                let check = cond * cond_inv;
                assert_eq!(check, GoldilocksField::from_noncanonical_i64(1_i64));
            }
        }

        #[test]
        fn test_unsigned(a in any::<u32>(), b in any::<u32>()) {
            let abs_diff = a.abs_diff(b);
            let a_field :GoldilocksField = GoldilocksField::from_noncanonical_i64(a as i64);
            let b_field : GoldilocksField = GoldilocksField::from_noncanonical_i64(b as i64);
            let abs_field: GoldilocksField = GoldilocksField::from_noncanonical_i64(abs_diff as i64);
            let cond  = a_field - b_field - abs_field;
            if a > b {
                assert_eq!(cond, GoldilocksField::from_noncanonical_i64(0_i64));
            } else {
                assert_ne!(cond, GoldilocksField::from_noncanonical_i64(0_i64));
                let cond_inv = cond.try_inverse().expect("can't inverse");
                let check = cond * cond_inv;
                assert_eq!(check, GoldilocksField::from_noncanonical_i64(1_i64));
            }
        }
    }
}
