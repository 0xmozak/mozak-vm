use itertools::Itertools;
use mozak_runner::state::Poseidon2Entry;
use mozak_runner::vm::Row;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::hash::hash_types::RichField;
use plonky2::hash::poseidon2::{Poseidon2, WIDTH};

use crate::poseidon2::columns::{Poseidon2State, ROUNDS_F, ROUNDS_P, STATE_SIZE};

struct FullRoundOutput<F> {
    pub state: [F; STATE_SIZE],
    pub s_box_input_qube: [F; STATE_SIZE],
}

struct PartialRoundOutput<F> {
    pub state_0: F,
    pub s_box_input_qube_0: F,
}

/// Pad the trace to a power of 2.
#[must_use]
fn pad_trace<F: RichField + Extendable<D>, const D: usize>(
    mut trace: Vec<Poseidon2State<F>>,
) -> Vec<Poseidon2State<F>> {
    let original_len = trace.len();
    let ext_trace_len = original_len.next_power_of_two();

    trace.resize(
        ext_trace_len,
        generate_poseidon2_state(&[F::ZERO; STATE_SIZE], false),
    );

    trace
}

fn x_qube<B: RichField, F: FieldExtension<D, BaseField = B>, const D: usize>(x: F) -> F {
    // x |--> x^3
    let x2 = x.square();
    x * x2
}

fn generate_1st_full_round_state<Field: RichField + Extendable<D>, const D: usize>(
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

fn generate_partial_round_state<Field: RichField + Extendable<D>, const D: usize>(
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

fn generate_2st_full_round_state<Field: RichField + Extendable<D>, const D: usize>(
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

pub fn generate_poseidon2_state<F: RichField + Extendable<D>, const D: usize>(
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
    for j in 0..STATE_SIZE {
        state.state_after_partial_rounds[j] = state_after_partial_rounds[j];
    }
    state
}

fn generate_poseidon2_states<F: RichField + Extendable<D>, const D: usize>(
    poseidon_data: &Poseidon2Entry<F>,
) -> Vec<Poseidon2State<F>> {
    poseidon_data
        .sponge_data
        .iter()
        .map(|(preimage, _output)| generate_poseidon2_state(preimage, true))
        .collect()
}

#[must_use]
pub fn generate_poseidon2_trace<F: RichField + Extendable<D>, const D: usize>(
    step_rows: &[Row<F>],
) -> Vec<Poseidon2State<F>> {
    let trace = pad_trace(
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
    );
    log::trace!("Poseison2 trace {:?}", trace);
    trace
}

#[cfg(test)]
mod test {
    use mozak_runner::state::{Aux, Poseidon2Entry};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Sample;
    use plonky2::hash::poseidon2::Poseidon2;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use crate::generation::poseidon2::{
        generate_1st_full_round_state, generate_2st_full_round_state, generate_partial_round_state,
        FullRoundOutput, Row,
    };
    use crate::poseidon2::columns::{Poseidon2State, ROUNDS_F, STATE_SIZE};
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn rounds_generation() {
        let preimage = (0..STATE_SIZE).map(|_| F::rand()).collect::<Vec<_>>();
        let output0: Vec<FullRoundOutput<F>> = generate_1st_full_round_state::<GoldilocksField, 2>(
            &preimage.clone().try_into().unwrap(),
        );
        let (_partial_state, state_after_partial_rounds) =
            generate_partial_round_state::<GoldilocksField, 2>(&output0.last().unwrap().state);
        let output2: Vec<FullRoundOutput<F>> =
            generate_2st_full_round_state::<GoldilocksField, 2>(&state_after_partial_rounds);
        let expected_output = <F as Poseidon2>::poseidon2(preimage.try_into().unwrap());
        assert_eq!(expected_output, output2.last().unwrap().state);
    }
    #[test]
    fn generate_poseidon2_trace() {
        let num_rows = 12;
        let mut step_rows = vec![];
        let mut sponge_data = vec![];

        for _ in 0..num_rows {
            let preimage = (0..STATE_SIZE).map(|_| F::rand()).collect::<Vec<_>>();
            // NOTE: this stark does not use output from sponge_data so its okay to pass all
            // ZERO as output
            sponge_data.push((
                preimage.try_into().expect("can't fail"),
                [F::default(); STATE_SIZE],
            ));
        }
        step_rows.push(Row {
            aux: Aux {
                poseidon2: Some(Poseidon2Entry::<F> {
                    addr: 0,
                    len: 0, // does not matter
                    sponge_data,
                }),
                ..Default::default()
            },
            ..Default::default()
        });

        let trace = super::generate_poseidon2_trace::<GoldilocksField, 2>(&step_rows);
        for step_row in step_rows.iter().take(num_rows) {
            let poseidon2 = step_row.aux.poseidon2.clone().expect("can't fail");
            for (i, (preimage, _output)) in poseidon2.sponge_data.iter().enumerate() {
                let expected_hash = <F as Poseidon2>::poseidon2(*preimage);
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
    #[test]
    fn generate_poseidon2_trace_with_dummy() {
        let step_rows = vec![];
        let trace: Vec<Poseidon2State<F>> =
            super::generate_poseidon2_trace::<GoldilocksField, 2>(&step_rows);
        assert_eq!(trace.len(), 1);
    }
}
