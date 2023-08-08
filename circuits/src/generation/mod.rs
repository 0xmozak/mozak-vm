pub mod bitshift;
pub mod bitwise;
pub mod cpu;
pub mod instruction;
pub mod memory;
pub mod rangecheck;

use mozak_vm::elf::Program;
use mozak_vm::vm::Row;
use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::{Field, Sample};
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use plonky2::util::transpose;
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

use self::bitshift::generate_shift_amount_trace;
use self::bitwise::generate_bitwise_trace;
use self::cpu::generate_cpu_trace;
use self::rangecheck::generate_rangecheck_trace;
use crate::bitshift::stark::BitshiftStark;
use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::stark::mozak_stark::NUM_TABLES;
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};

#[must_use]
pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    program: &Program,
    step_rows: &[Row],
) -> [Vec<PolynomialValues<F>>; NUM_TABLES] {
    let cpu_rows = generate_cpu_trace::<F>(program, step_rows);
    let rangecheck_rows = generate_rangecheck_trace::<F>(&cpu_rows);
    let bitwise_rows = generate_bitwise_trace(&cpu_rows);
    let shift_amount_rows = generate_shift_amount_trace(&cpu_rows);

    let cpu_trace = trace_rows_to_poly_values(cpu_rows);
    let rangecheck_trace = trace_to_poly_values(rangecheck_rows);
    let bitwise_trace = trace_rows_to_poly_values(bitwise_rows);
    let shift_amount_trace = trace_rows_to_poly_values(shift_amount_rows);
    [
        cpu_trace,
        rangecheck_trace,
        bitwise_trace,
        shift_amount_trace,
    ]
}

#[must_use]
#[allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]
pub fn generate_traces_debug(program: &Program, step_rows: &[Row]) -> bool {
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type CpuStarkT = CpuStark<F, D>;
    type RcStarkT = RangeCheckStark<F, D>;
    type BtStarkT = BitwiseStark<F, D>;
    type BsStarkT = BitshiftStark<F, D>;

    let mut acc = F::ZERO;
    ////////////////////////////////////////////////////////////////////
    //////////////////////// CPU DEBUG TRACE ///////////////////////////
    //////////////////////// ///////////////////////////////////////////
    let cpu_rows_debug = generate_cpu_trace::<F>(program, step_rows);
    let mut cpu_consumer_debug = ConstraintConsumer::new_debug(vec![F::rand()]);
    let cpu_stark = CpuStarkT::default();

    for i in 1..cpu_rows_debug.len() {
        let mut lv: Vec<_> = vec![];
        cpu_rows_debug[i - 1].into_iter().for_each(|e| {
            lv.push(e);
        });

        let mut nv: Vec<_> = vec![];
        cpu_rows_debug[i].into_iter().for_each(|e| {
            nv.push(e);
        });

        if i == 1 {
            cpu_consumer_debug.debug_activate_first_row();
        } else if i == cpu_rows_debug.len() - 1 {
            cpu_consumer_debug.debug_activate_last_row();
            lv = nv.clone();
        } else {
            cpu_consumer_debug.debug_activate_transition();
        }

        cpu_stark.eval_packed_generic(
            StarkEvaluationVars {
                local_values: lv.as_slice().try_into().unwrap(),
                next_values: nv.as_slice().try_into().unwrap(),
                public_inputs: &[F::ZERO; CpuStarkT::PUBLIC_INPUTS],
            },
            &mut cpu_consumer_debug,
        );
        cpu_consumer_debug.constraint_accs.iter().for_each(|e| {
            acc += *e;
            assert!(e.eq(&F::ZERO));
        });
    }
    assert!(acc.eq(&F::ZERO));
    ///////////////////////////////////////////////////////////////////////////
    //////////////////////// RC DEBUG TRACE ///////////////////////////////////
    //////////////////////// //////////////////////////////////////////////////
    let rc_rows_debug_trace = generate_rangecheck_trace::<F>(&cpu_rows_debug);
    let rc_rows_debug = transpose(&rc_rows_debug_trace);
    let mut rc_consumer_debug = ConstraintConsumer::new_debug(vec![F::rand()]);
    let rc_stark = RcStarkT::default();

    for i in 1..rc_rows_debug.len() {
        let mut lv: Vec<_> = vec![];
        rc_rows_debug[i - 1].iter().for_each(|e| lv.push(*e));

        let mut nv: Vec<_> = vec![];
        rc_rows_debug[i].iter().for_each(|e| nv.push(*e));

        if i == 1 {
            rc_consumer_debug.debug_activate_first_row();
        } else if i == rc_rows_debug.len() - 1 {
            rc_consumer_debug.debug_activate_last_row();
            lv = nv.clone();
            nv.clear();
            rc_rows_debug[0].iter().for_each(|e| nv.push(*e));
        } else {
            rc_consumer_debug.debug_activate_transition();
        }

        let local_val: &[GoldilocksField; RcStarkT::COLUMNS] = lv.as_slice().try_into().unwrap();
        let next_val: &[GoldilocksField; RcStarkT::COLUMNS] = nv.as_slice().try_into().unwrap();

        rc_stark.eval_packed_generic(
            StarkEvaluationVars {
                local_values: local_val,
                next_values: next_val,
                public_inputs: &[F::ZERO; RcStarkT::PUBLIC_INPUTS],
            },
            &mut rc_consumer_debug,
        );
        rc_consumer_debug.constraint_accs.iter().for_each(|e| {
            acc += *e;
            assert!(e.eq(&F::ZERO));
        });
    }
    assert!(acc.eq(&F::ZERO));

    ///////////////////////////////////////////////////////////////////////////
    //////////////////////// BT DEBUG TRACE ///////////////////////////////////
    //////////////////////// //////////////////////////////////////////////////
    let bt_rows_debug = generate_bitwise_trace::<F>(&cpu_rows_debug);
    let mut bt_consumer_debug = ConstraintConsumer::new_debug(vec![F::rand()]);
    let bt_stark = BtStarkT::default();

    for i in 1..bt_rows_debug.len() {
        let mut lv: Vec<_> = vec![];
        bt_rows_debug[i - 1].into_iter().for_each(|e| {
            lv.push(e);
        });

        let mut nv: Vec<_> = vec![];
        bt_rows_debug[i].into_iter().for_each(|e| {
            nv.push(e);
        });

        if i == 1 {
            bt_consumer_debug.debug_activate_first_row();
        } else if i == bt_rows_debug.len() - 1 {
            bt_consumer_debug.debug_activate_last_row();
            lv = nv.clone();
        } else {
            bt_consumer_debug.debug_activate_transition();
        }
        bt_stark.eval_packed_generic(
            StarkEvaluationVars {
                local_values: lv.as_slice().try_into().unwrap(),
                next_values: nv.as_slice().try_into().unwrap(),
                public_inputs: &[F::ZERO; BtStarkT::PUBLIC_INPUTS],
            },
            &mut bt_consumer_debug,
        );
        bt_consumer_debug.constraint_accs.iter().for_each(|e| {
            acc += *e;
            assert!(e.eq(&F::ZERO));
        });
    }
    assert!(acc.eq(&F::ZERO));

    ///////////////////////////////////////////////////////////////////////////
    //////////////////////// BS DEBUG TRACE ///////////////////////////////////
    //////////////////////// //////////////////////////////////////////////////
    let bitshift_rows_debug = generate_shift_amount_trace::<F>(&cpu_rows_debug);
    let mut bitshift_consumer_debug = ConstraintConsumer::new_debug(vec![F::rand()]);
    let bitshift_stark = BsStarkT::default();

    for i in 1..bitshift_rows_debug.len() {
        let mut lv: Vec<_> = vec![];
        bitshift_rows_debug[i - 1].into_iter().for_each(|e| {
            lv.push(e);
        });

        let mut nv: Vec<_> = vec![];
        bitshift_rows_debug[i].into_iter().for_each(|e| {
            nv.push(e);
        });

        if i == 1 {
            bitshift_consumer_debug.debug_activate_first_row();
        } else if i == bitshift_rows_debug.len() - 1 {
            bitshift_consumer_debug.debug_activate_last_row();
            lv = nv.clone();
        } else {
            bitshift_consumer_debug.debug_activate_transition();
        }
        bitshift_stark.eval_packed_generic(
            StarkEvaluationVars {
                local_values: lv.as_slice().try_into().unwrap(),
                next_values: nv.as_slice().try_into().unwrap(),
                public_inputs: &[F::ZERO; BsStarkT::PUBLIC_INPUTS],
            },
            &mut bitshift_consumer_debug,
        );
        bitshift_consumer_debug
            .constraint_accs
            .iter()
            .for_each(|e| {
                acc += *e;
                assert!(e.eq(&F::ZERO));
            });
    }
    assert!(acc.eq(&F::ZERO));
    acc.eq(&F::ZERO)
}
