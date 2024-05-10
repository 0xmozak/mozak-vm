use itertools::Itertools;
use mozak_runner::poseidon2;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::poseidon2::{Poseidon2, WIDTH};

use crate::poseidon2::columns::{Poseidon2State, ROUNDS_F, ROUNDS_P, STATE_SIZE};
use crate::utils::pad_trace_with_row;

struct FullRoundOutput<F> {
    pub state: [F; STATE_SIZE],
    pub s_box_input_qube: [F; STATE_SIZE],
}

struct PartialRoundOutput<F> {
    pub state_0: F,
    pub s_box_input_qube_0: F,
}

fn x_qube<F: RichField>(x: F) -> F {
    // x |--> x^3
    x * x * x
}

fn generate_1st_full_round_state<Field: RichField>(
    preimage: &[Field; STATE_SIZE],
) -> Vec<FullRoundOutput<Field>> {
    let mut outputs = Vec::new();
    assert_eq!(STATE_SIZE, WIDTH);
    let mut current_state = *preimage;

    // Linear layer at start
    Field::matmul_external(&mut current_state);

    for r in 0..(ROUNDS_F / 2) {
        <Field as Poseidon2>::constant_layer(&mut current_state, r);
        let mut s_box_input_qube = current_state;
        s_box_input_qube
            .iter_mut()
            .for_each(|x: &mut Field| *x = x_qube(*x));
        <Field as Poseidon2>::sbox_layer(&mut current_state);
        Field::matmul_external(&mut current_state);
        outputs.push(FullRoundOutput {
            state: current_state,
            s_box_input_qube,
        });
    }

    outputs
}

fn generate_partial_round_state<Field: RichField>(
    last_rount_output: &[Field; STATE_SIZE],
) -> (Vec<PartialRoundOutput<Field>>, [Field; STATE_SIZE]) {
    let mut outputs = Vec::new();
    assert_eq!(STATE_SIZE, WIDTH);
    let mut current_state = *last_rount_output;

    for r in 0..ROUNDS_P {
        current_state[0] += Field::from_canonical_u64(<Field as Poseidon2>::RC12_MID[r]);
        let s_box_input_qube_0 = current_state[0] * current_state[0] * current_state[0];
        current_state[0] = <Field as Poseidon2>::sbox_monomial(current_state[0]);
        Field::matmul_internal(&mut current_state, &<Field as Poseidon2>::MAT_DIAG12_M_1);
        outputs.push(PartialRoundOutput {
            state_0: current_state[0],
            s_box_input_qube_0,
        });
    }

    (outputs, current_state)
}

fn generate_2st_full_round_state<Field: RichField>(
    last_rount_output: &[Field; STATE_SIZE],
) -> Vec<FullRoundOutput<Field>> {
    let mut outputs = Vec::new();
    assert_eq!(STATE_SIZE, WIDTH);
    let mut current_state = *last_rount_output;

    for r in (ROUNDS_F / 2)..ROUNDS_F {
        <Field as Poseidon2>::constant_layer(&mut current_state, r);
        let mut s_box_input_qube = current_state;
        s_box_input_qube
            .iter_mut()
            .for_each(|x: &mut Field| *x = x.square() * (*x));
        <Field as Poseidon2>::sbox_layer(&mut current_state);
        Field::matmul_external(&mut current_state);
        outputs.push(FullRoundOutput {
            state: current_state,
            s_box_input_qube,
        });
    }

    outputs
}

pub fn generate_poseidon2_state<F: RichField>(
    preimage: &[F; STATE_SIZE],
    is_exe: bool,
) -> Poseidon2State<F> {
    let mut state = Poseidon2State::<F> {
        is_exe: if is_exe { F::ONE } else { F::ZERO },
        input: *preimage,
        ..Default::default()
    };
    let first_full_round_state = generate_1st_full_round_state(preimage);
    let (partial_round_state, state_after_partial_rounds) =
        generate_partial_round_state(&first_full_round_state.last().unwrap().state);
    let second_full_round_state = generate_2st_full_round_state(&state_after_partial_rounds);
    for j in 0..(ROUNDS_F / 2) {
        for k in 0..STATE_SIZE {
            state.state_after_first_full_rounds[j * STATE_SIZE + k] =
                first_full_round_state[j].state[k];
            state.s_box_input_qube_first_full_rounds[j * STATE_SIZE + k] =
                first_full_round_state[j].s_box_input_qube[k];
            state.state_after_second_full_rounds[j * STATE_SIZE + k] =
                second_full_round_state[j].state[k];
            state.s_box_input_qube_second_full_rounds[j * STATE_SIZE + k] =
                second_full_round_state[j].s_box_input_qube[k];
        }
    }
    for (j, partial_round_state) in partial_round_state.iter().enumerate().take(ROUNDS_P) {
        state.state0_after_partial_rounds[j] = partial_round_state.state_0;
        state.s_box_input_qube_partial_rounds[j] = partial_round_state.s_box_input_qube_0;
    }
    state.state_after_partial_rounds[..STATE_SIZE]
        .copy_from_slice(&state_after_partial_rounds[..STATE_SIZE]);
    state
}

fn generate_poseidon2_states<F: RichField>(
    poseidon_data: &poseidon2::Entry<F>,
) -> Vec<Poseidon2State<F>> {
    poseidon_data
        .sponge_data
        .iter()
        .map(|sponge_datum| generate_poseidon2_state(&sponge_datum.preimage, true))
        .collect()
}

#[must_use]
pub fn generate_poseidon2_trace<F: RichField>(step_rows: &[Row<F>]) -> Vec<Poseidon2State<F>> {
    let trace = pad_trace_with_row(
        step_rows
            .iter()
            .filter(|row| row.aux.poseidon2.is_some())
            .map(|s| {
                let poseidon_data = s.aux.poseidon2.clone().expect("can't fail");
                generate_poseidon2_states(&poseidon_data)
            })
            .collect_vec()
            .into_iter()
            .flatten()
            .collect::<Vec<Poseidon2State<F>>>(),
        generate_poseidon2_state(&[F::ZERO; STATE_SIZE], false),
    );
    log::trace!("Poseidon2 trace {:?}", trace);
    log::info!("Poseidon2 trace length {:?}", trace.len());
    trace
}

#[cfg(test)]
mod test {

    use plonky2::field::types::Sample;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use super::*;
    use crate::generation::MIN_TRACE_LENGTH;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn rounds_generation() {
        let preimage = (0..STATE_SIZE).map(|_| F::rand()).collect::<Vec<_>>();
        let output0: Vec<FullRoundOutput<F>> =
            generate_1st_full_round_state(&preimage.clone().try_into().unwrap());
        let (_partial_state, state_after_partial_rounds) =
            generate_partial_round_state(&output0.last().unwrap().state);
        let output2: Vec<FullRoundOutput<F>> =
            generate_2st_full_round_state(&state_after_partial_rounds);
        let expected_output = <F as Poseidon2>::poseidon2(preimage.try_into().unwrap());
        assert_eq!(expected_output, output2.last().unwrap().state);
    }
    #[test]
    fn generate_poseidon2_trace() {
        let (_program, record) = create_poseidon2_test(&[Poseidon2Test {
            data: "😇 Mozak is knowledge arguments based technology".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }]);

        let step_rows = record.executed;
        let trace = super::generate_poseidon2_trace(&step_rows);
        for step_row in &step_rows {
            if let Some(poseidon2) = step_row.aux.poseidon2.as_ref() {
                for (i, sponge_datum) in poseidon2.sponge_data.iter().enumerate() {
                    let expected_hash = <F as Poseidon2>::poseidon2(sponge_datum.preimage);
                    for (j, expected_hash) in expected_hash.iter().enumerate().take(STATE_SIZE) {
                        assert_eq!(
                            *expected_hash,
                            trace[i].state_after_second_full_rounds
                                [STATE_SIZE * (ROUNDS_F / 2 - 1) + j],
                            "Mismatch at row {i}, position {j}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn generate_poseidon2_trace_with_dummy() {
        let step_rows = vec![];
        let trace: Vec<Poseidon2State<F>> = super::generate_poseidon2_trace(&step_rows);
        assert_eq!(trace.len(), MIN_TRACE_LENGTH);
    }
}
