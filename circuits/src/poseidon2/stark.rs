use std::marker::PhantomData;

use expr::{Expr, ExprBuilder, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::poseidon2::Poseidon2;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use super::columns::Poseidon2State;
use crate::columns_view::HasNamedColumns;
use crate::expr::{build_ext, build_packed, ConstraintBuilder};
use crate::poseidon2::columns::{NUM_POSEIDON2_COLS, ROUNDS_F, ROUNDS_P, STATE_SIZE};
use crate::unstark::NoColumns;

// degree: 1
fn add_rc<'a, V, W, const STATE_SIZE: usize>(
    state: &mut [Expr<'a, V>; STATE_SIZE],
    r: usize,
    from_u64: &mut impl FnMut(u64) -> Expr<'a, V>,
) where
    V: Copy,
    W: Poseidon2, {
    assert_eq!(STATE_SIZE, 12);

    for (i, val) in state.iter_mut().enumerate().take(STATE_SIZE) {
        *val = *val + from_u64(<W as Poseidon2>::RC12[r + i]);
    }
}

// degree: 3
fn sbox_p<'a, V>(x: &mut Expr<'a, V>, x_qube: &Expr<'a, V>)
where
    V: Copy, {
    *x = *x_qube * *x_qube * *x;
}

fn matmul_m4<V, const STATE_SIZE: usize>(state: &mut [Expr<'_, V>; STATE_SIZE])
where
    V: Copy, {
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
        let x1_2 = state[start_index + 1] * 2;
        // 2x3
        let x3_2 = state[start_index + 3] * 2;

        // t2 = 2x1 + t1
        let t_2 = x1_2 + t_1;

        // t3 = 2x3 + t0
        let t_3 = x3_2 + t_0;

        // t4 = 4t1 + t3
        let t_4 = t_3 + t_1 * 4;

        // t5 = 4t0 + t2
        let t_5 = t_2 + t_0 * 4;

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

fn matmul_external12<V>(state: &mut [Expr<'_, V>; STATE_SIZE])
where
    V: Copy, {
    assert_eq!(STATE_SIZE, 12);
    matmul_m4(state);

    let t4 = STATE_SIZE / 4;
    let mut stored = [Expr::from(0); 4];

    for l in 0..4 {
        stored[l] = state[l];
        for j in 1..t4 {
            // TODO: Update once AddAssign is implemented
            stored[l] = stored[l] + state[4 * j + l];
        }
    }
    for i in 0..STATE_SIZE {
        state[i] = state[i] + stored[i % 4];
    }
}

// degree: 1
fn matmul_internal12<'a, V, U, const STATE_SIZE: usize>(
    state: &mut [Expr<'a, V>; STATE_SIZE],
    from_u64: &mut impl FnMut(u64) -> Expr<'a, V>,
) where
    V: Copy,
    U: Poseidon2, {
    assert_eq!(STATE_SIZE, 12);
    // TODO: Replace this with an implementation of Sum trait
    let mut sum = Expr::from(0);
    for item in &mut *state {
        sum = sum + *item;
    }

    for (i, val) in state.iter_mut().enumerate().take(STATE_SIZE) {
        // TODO: Fix once MulAssign is implemented
        *val = *val * (from_u64(<U as Poseidon2>::MAT_DIAG12_M_1[i]) - 1);
        *val = *val + sum;
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
const PUBLIC_INPUTS: usize = 0;

// NOTE: This one has extra constraints...
fn generate_constraints<'a, V: Copy, U: Poseidon2>(
    vars: &StarkFrameTyped<Poseidon2State<Expr<'a, V>>, NoColumns<Expr<'a, V>>>,
    from_u64: &mut impl FnMut(u64) -> Expr<'a, V>,
) -> ConstraintBuilder<Expr<'a, V>> {
    let lv = vars.local_values;
    let mut constraints = ConstraintBuilder::default();

    // row can be execution or padding.
    constraints.always(lv.is_exe.is_binary());

    let mut state = lv.input;
    matmul_external12(&mut state);
    // first full rounds
    for r in 0..(ROUNDS_F / 2) {
        add_rc::<V, U, STATE_SIZE>(&mut state, r, from_u64);
        for (i, item) in state.iter_mut().enumerate().take(STATE_SIZE) {
            sbox_p(
                item,
                &lv.s_box_input_qube_first_full_rounds[r * STATE_SIZE + i],
            );
        }
        matmul_external12(&mut state);
        for (i, state_i) in state.iter_mut().enumerate().take(STATE_SIZE) {
            constraints.always(*state_i - lv.state_after_first_full_rounds[r * STATE_SIZE + i]);
            *state_i = lv.state_after_first_full_rounds[r * STATE_SIZE + i];
        }
    }

    // partial rounds
    for i in 0..ROUNDS_P {
        state[0] = state[0] + from_u64(<U as Poseidon2>::RC12_MID[i]);
        sbox_p(&mut state[0], &lv.s_box_input_qube_partial_rounds[i]);
        matmul_internal12::<V, U, STATE_SIZE>(&mut state, from_u64);
        constraints.always(state[0] - lv.state0_after_partial_rounds[i]);
        state[0] = lv.state0_after_partial_rounds[i];
    }

    // the state before last full rounds
    for (i, state_i) in state.iter_mut().enumerate().take(STATE_SIZE) {
        constraints.always(*state_i - lv.state_after_partial_rounds[i]);
        *state_i = lv.state_after_partial_rounds[i];
    }

    // last full rounds
    for i in 0..(ROUNDS_F / 2) {
        let r = (ROUNDS_F / 2) + i;
        add_rc::<V, U, STATE_SIZE>(&mut state, r, from_u64);
        for (j, item) in state.iter_mut().enumerate().take(STATE_SIZE) {
            sbox_p(
                item,
                &lv.s_box_input_qube_second_full_rounds[i * STATE_SIZE + j],
            );
        }
        matmul_external12(&mut state);
        for (j, state_j) in state.iter_mut().enumerate().take(STATE_SIZE) {
            constraints.always(*state_j - lv.state_after_second_full_rounds[i * STATE_SIZE + j]);
            *state_j = lv.state_after_second_full_rounds[i * STATE_SIZE + j];
        }
    }

    constraints
}

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
        consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let eb = ExprBuilder::default();
        let constraints = generate_constraints::<P, F>(&eb.to_typed_starkframe(vars), &mut |u| {
            eb.lit(P::from(FE::from_basefield(F::from_canonical_u64(u))))
        });
        build_packed(constraints, consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let constraints = generate_constraints::<ExtensionTarget<D>, F>(
            &eb.to_typed_starkframe(vars),
            &mut |u| eb.lit(builder.constant_extension(F::Extension::from_canonical_u64(u))),
        );
        build_ext(constraints, builder, consumer);
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

    use crate::poseidon2::generation::generate_poseidon2_trace;
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
