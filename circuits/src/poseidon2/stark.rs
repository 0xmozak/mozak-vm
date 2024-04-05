use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::poseidon2::Poseidon2;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::Poseidon2State;
use crate::columns_view::HasNamedColumns;
use crate::poseidon2::columns::{NUM_POSEIDON2_COLS, ROUNDS_F, ROUNDS_P, STATE_SIZE};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

// degree: 1
fn add_rc_constraints<
    F: RichField + Extendable<D>,
    const D: usize,
    FE,
    P,
    const D2: usize,
    const STATE_SIZE: usize,
>(
    state: &mut [P; STATE_SIZE],
    r: usize,
) where
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    assert_eq!(STATE_SIZE, 12);

    for (i, val) in state.iter_mut().enumerate().take(STATE_SIZE) {
        *val += FE::from_basefield(F::from_canonical_u64(<F as Poseidon2>::RC12[r + i]));
    }
}

// degree: 3
fn sbox_p_constraints<F: RichField + Extendable<D>, const D: usize, FE, P, const D2: usize>(
    x: &mut P,
    x_qube: &P,
) where
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    *x = *x_qube * *x_qube * *x;
}

fn matmul_m4_constraints<
    F: RichField + Extendable<D>,
    const D: usize,
    FE,
    P,
    const D2: usize,
    const STATE_SIZE: usize,
>(
    state: &mut [P; STATE_SIZE],
) where
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    // input x = (x0, x1, x2, x3)
    assert_eq!(STATE_SIZE, 12);
    let t4 = STATE_SIZE / 4;
    for i in 0..t4 {
        let start_index = i * 4;
        // t0 = x0 + x1
        let t_0 = state[start_index] + state[start_index + 1];

        // t1 = x2 + x3
        let t_1 = state[start_index + 2] + state[start_index + 3];

        // 2x1
        let x1_2 = state[start_index + 1].mul(FE::TWO);
        // 2x3
        let x3_2 = state[start_index + 3].mul(FE::TWO);
        let four = FE::TWO + FE::TWO;

        // t2 = 2x1 + t1
        let t_2 = x1_2 + t_1;

        // t3 = 2x3 + t0
        let t_3 = x3_2 + t_0;

        // t4 = 4t1 + t3
        let t_4 = t_3 + t_1.mul(four);

        // t5 = 4t0 + t2
        let t_5 = t_2 + t_0.mul(four);

        // t6 = t3 + t5
        let t_6 = t_3 + t_5;

        // t7 = t2 + t4
        let t_7 = t_2 + t_4;

        state[start_index] = t_6;
        state[start_index + 1] = t_5;
        state[start_index + 2] = t_7;
        state[start_index + 3] = t_4;
    }
}

fn matmul_external12_constraints<
    F: RichField + Extendable<D>,
    const D: usize,
    FE,
    P,
    const D2: usize,
    const STATE_SIZE: usize,
>(
    state: &mut [P; STATE_SIZE],
) where
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    assert_eq!(STATE_SIZE, 12);
    matmul_m4_constraints(state);

    let t4 = STATE_SIZE / 4;
    let mut stored = [P::ZEROS; 4];

    for l in 0..4 {
        stored[l] = state[l];
        for j in 1..t4 {
            stored[l] += state[4 * j + l];
        }
    }
    for i in 0..STATE_SIZE {
        state[i] = state[i].add(stored[i % 4]);
    }
}

// degree: 1
fn matmul_internal12_constraints<
    F: RichField + Extendable<D>,
    const D: usize,
    FE,
    P,
    const D2: usize,
    const STATE_SIZE: usize,
>(
    state: &mut [P; STATE_SIZE],
) where
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    assert_eq!(STATE_SIZE, 12);
    let mut sum = P::ZEROS;

    for item in &mut *state {
        sum += *item;
    }

    for (i, val) in state.iter_mut().enumerate().take(STATE_SIZE) {
        *val *= FE::from_basefield(F::from_canonical_u64(
            <F as Poseidon2>::MAT_DIAG12_M_1[i] - 1,
        ));
        *val += sum;
    }
}

fn add_rc_circuit<F: RichField + Extendable<D>, const D: usize, const STATE_SIZE: usize>(
    builder: &mut CircuitBuilder<F, D>,
    state: &mut [ExtensionTarget<D>; STATE_SIZE],
    r: usize,
) {
    assert_eq!(STATE_SIZE, 12);

    for (i, val) in state.iter_mut().enumerate().take(STATE_SIZE) {
        let round_const = F::Extension::from_canonical_u64(<F as Poseidon2>::RC12[r + i]);
        let rc_ext = builder.constant_extension(round_const);
        *val = builder.add_extension(*val, rc_ext);
    }
}

fn sbox_p_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    x: &mut ExtensionTarget<D>,
    x_qube: &ExtensionTarget<D>,
) {
    *x = builder.mul_many_extension([*x_qube, *x_qube, *x]);
}

fn matmul_m4_circuit<F: RichField + Extendable<D>, const D: usize, const STATE_SIZE: usize>(
    builder: &mut CircuitBuilder<F, D>,
    state: &mut [ExtensionTarget<D>; STATE_SIZE],
) {
    // input x = (x0, x1, x2, x3)
    assert_eq!(STATE_SIZE, 12);
    let t4 = STATE_SIZE / 4;
    for i in 0..t4 {
        let start_index = i * 4;
        // t0 = x0 + x1
        let t_0 =
            builder.mul_const_add_extension(F::ONE, state[start_index], state[start_index + 1]);

        // t1 = x2 + x3
        let t_1 =
            builder.mul_const_add_extension(F::ONE, state[start_index + 2], state[start_index + 3]);

        let four = F::TWO + F::TWO;

        // t2 = 2x1 + t1
        let t_2 = builder.mul_const_add_extension(F::TWO, state[start_index + 1], t_1);

        // t3 = 2x3 + t0
        let t_3 = builder.mul_const_add_extension(F::TWO, state[start_index + 3], t_0);

        // t4 = 4t1 + t3
        let t_4 = builder.mul_const_add_extension(four, t_1, t_3);

        // t5 = 4t0 + t2
        let t_5 = builder.mul_const_add_extension(four, t_0, t_2);

        // t6 = t3 + t5
        let t_6 = builder.mul_const_add_extension(F::ONE, t_3, t_5);

        // t7 = t2 + t4
        let t_7 = builder.mul_const_add_extension(F::ONE, t_2, t_4);

        state[start_index] = t_6;
        state[start_index + 1] = t_5;
        state[start_index + 2] = t_7;
        state[start_index + 3] = t_4;
    }
}

fn matmul_external12_circuit<
    F: RichField + Extendable<D>,
    const D: usize,
    const STATE_SIZE: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    state: &mut [ExtensionTarget<D>; STATE_SIZE],
) {
    assert_eq!(STATE_SIZE, 12);
    matmul_m4_circuit(builder, state);
    let mut temp = [builder.zero_extension(); STATE_SIZE];
    temp[0] = builder.add_many_extension([state[0], state[0], state[4], state[8]]);
    temp[1] = builder.add_many_extension([state[1], state[1], state[5], state[9]]);
    temp[2] = builder.add_many_extension([state[2], state[2], state[6], state[10]]);
    temp[3] = builder.add_many_extension([state[3], state[3], state[7], state[11]]);

    temp[4] = builder.add_many_extension([state[4], state[0], state[4], state[8]]);
    temp[5] = builder.add_many_extension([state[5], state[1], state[5], state[9]]);
    temp[6] = builder.add_many_extension([state[6], state[2], state[6], state[10]]);
    temp[7] = builder.add_many_extension([state[7], state[3], state[7], state[11]]);

    temp[8] = builder.add_many_extension([state[8], state[0], state[4], state[8]]);
    temp[9] = builder.add_many_extension([state[9], state[1], state[5], state[9]]);
    temp[10] = builder.add_many_extension([state[10], state[2], state[6], state[10]]);
    temp[11] = builder.add_many_extension([state[11], state[3], state[7], state[11]]);

    *state = temp;
}

fn matmul_internal12_circuit<
    F: RichField + Extendable<D>,
    const D: usize,
    const STATE_SIZE: usize,
>(
    builder: &mut CircuitBuilder<F, D>,
    state: &mut [ExtensionTarget<D>; STATE_SIZE],
) {
    assert_eq!(STATE_SIZE, 12);
    let sum = builder.add_many_extension(*state);

    for (i, val) in state.iter_mut().enumerate().take(STATE_SIZE) {
        let round_const = F::Extension::from_canonical_u64(<F as Poseidon2>::MAT_DIAG12_M_1[i] - 1);
        let round_const_ext = builder.constant_extension(round_const);
        *val = builder.mul_add_extension(round_const_ext, *val, sum);
    }
}

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2_12Stark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for Poseidon2_12Stark<F, D> {
    type Columns = Poseidon2State<F>;
}

const COLUMNS: usize = NUM_POSEIDON2_COLS;
const PUBLIC_INPUTS: usize = 1;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for Poseidon2_12Stark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>
        where
            FE: FieldExtension<D2, BaseField = F>,
            P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &Poseidon2State<P> = vars.get_local_values().into();

        // row can be execution or padding.
        is_binary(yield_constr, lv.is_exe);

        let mut state = lv.input;
        matmul_external12_constraints(&mut state);
        // first full rounds
        for r in 0..(ROUNDS_F / 2) {
            add_rc_constraints(&mut state, r);
            for (i, item) in state.iter_mut().enumerate().take(STATE_SIZE) {
                sbox_p_constraints(
                    item,
                    &lv.s_box_input_qube_first_full_rounds[r * STATE_SIZE + i],
                );
            }
            matmul_external12_constraints(&mut state);
            for (i, state_i) in state.iter_mut().enumerate().take(STATE_SIZE) {
                yield_constr
                    .constraint(*state_i - lv.state_after_first_full_rounds[r * STATE_SIZE + i]);
                *state_i = lv.state_after_first_full_rounds[r * STATE_SIZE + i];
            }
        }

        // partial rounds
        for i in 0..ROUNDS_P {
            state[0] += FE::from_basefield(F::from_canonical_u64(<F as Poseidon2>::RC12_MID[i]));
            sbox_p_constraints(&mut state[0], &lv.s_box_input_qube_partial_rounds[i]);
            matmul_internal12_constraints(&mut state);
            yield_constr.constraint(state[0] - lv.state0_after_partial_rounds[i]);
            state[0] = lv.state0_after_partial_rounds[i];
        }

        // the state before last full rounds
        for (i, state_i) in state.iter_mut().enumerate().take(STATE_SIZE) {
            yield_constr.constraint(*state_i - lv.state_after_partial_rounds[i]);
            *state_i = lv.state_after_partial_rounds[i];
        }

        // last full rounds
        for i in 0..(ROUNDS_F / 2) {
            let r = (ROUNDS_F / 2) + i;
            add_rc_constraints(&mut state, r);
            for (j, item) in state.iter_mut().enumerate().take(STATE_SIZE) {
                sbox_p_constraints(
                    item,
                    &lv.s_box_input_qube_second_full_rounds[i * STATE_SIZE + j],
                );
            }
            matmul_external12_constraints(&mut state);
            for (j, state_j) in state.iter_mut().enumerate().take(STATE_SIZE) {
                yield_constr
                    .constraint(*state_j - lv.state_after_second_full_rounds[i * STATE_SIZE + j]);
                *state_j = lv.state_after_second_full_rounds[i * STATE_SIZE + j];
            }
        }
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &Poseidon2State<ExtensionTarget<D>> = vars.get_local_values().into();
        // row can be execution or padding.
        is_binary_ext_circuit(builder, lv.is_exe, yield_constr);

        let mut state = lv.input;
        matmul_external12_circuit(builder, &mut state);
        // first full rounds
        for r in 0..(ROUNDS_F / 2) {
            add_rc_circuit(builder, &mut state, r);
            for (i, item) in state.iter_mut().enumerate().take(STATE_SIZE) {
                sbox_p_circuit(
                    builder,
                    item,
                    &lv.s_box_input_qube_first_full_rounds[r * STATE_SIZE + i],
                );
            }
            matmul_external12_circuit(builder, &mut state);
            for (i, state_i) in state.iter_mut().enumerate().take(STATE_SIZE) {
                let sub_ext = builder.sub_extension(
                    *state_i,
                    lv.state_after_first_full_rounds[r * STATE_SIZE + i],
                );
                yield_constr.constraint(builder, sub_ext);
                *state_i = lv.state_after_first_full_rounds[r * STATE_SIZE + i];
            }
        }

        // partial rounds
        for i in 0..ROUNDS_P {
            let round_const_ext = builder.constant_extension(F::Extension::from_canonical_u64(
                <F as Poseidon2>::RC12_MID[i],
            ));
            state[0] = builder.add_extension(state[0], round_const_ext);
            sbox_p_circuit(
                builder,
                &mut state[0],
                &lv.s_box_input_qube_partial_rounds[i],
            );
            matmul_internal12_circuit(builder, &mut state);
            let sub_ext = builder.sub_extension(state[0], lv.state0_after_partial_rounds[i]);
            yield_constr.constraint(builder, sub_ext);
            state[0] = lv.state0_after_partial_rounds[i];
        }

        // the state before last full rounds
        for (i, state_i) in state.iter_mut().enumerate().take(STATE_SIZE) {
            let sub_ext = builder.sub_extension(*state_i, lv.state_after_partial_rounds[i]);
            yield_constr.constraint(builder, sub_ext);
            *state_i = lv.state_after_partial_rounds[i];
        }

        // last full rounds
        for i in 0..(ROUNDS_F / 2) {
            let r = (ROUNDS_F / 2) + i;
            add_rc_circuit(builder, &mut state, r);
            for (j, item) in state.iter_mut().enumerate().take(STATE_SIZE) {
                sbox_p_circuit(
                    builder,
                    item,
                    &lv.s_box_input_qube_second_full_rounds[i * STATE_SIZE + j],
                );
            }
            matmul_external12_circuit(builder, &mut state);
            for (j, state_j) in state.iter_mut().enumerate().take(STATE_SIZE) {
                let sub_ext = builder.sub_extension(
                    *state_j,
                    lv.state_after_second_full_rounds[i * STATE_SIZE + j],
                );
                yield_constr.constraint(builder, sub_ext);
                *state_j = lv.state_after_second_full_rounds[i * STATE_SIZE + j];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::prover::prove;
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use crate::generation::poseidon2::generate_poseidon2_trace;
    use crate::poseidon2::stark::Poseidon2_12Stark;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2_12Stark<F, D>;

    #[test]
    fn poseidon2_constraints() -> Result<()> {
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;
        config.fri_config.rate_bits = 3; // to meet the constraint degree bound

        let (_program, record) = create_poseidon2_test(&[Poseidon2Test {
            data: "😇 Mozak is knowledge arguments based technology".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }]);

        let step_rows = record.executed;

        let stark = S::default();
        let trace = generate_poseidon2_trace(&step_rows);
        let trace_poly_values = trace_rows_to_poly_values(trace);

        let proof = prove::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;
        verify_stark_proof(stark, proof, &config)
    }

    #[test]
    fn poseidon2_stark_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }
    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;
        Ok(())
    }
}
