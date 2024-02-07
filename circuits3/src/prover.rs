use itertools::Itertools;
use p3_air::{Air, AirBuilder, BaseAir};
use p3_challenger::{CanObserve, FieldChallenger};
use p3_commit::{Pcs, UnivariatePcs};
use p3_field::{Field, TwoAdicField};
use p3_matrix::dense::RowMajorMatrix;
use p3_matrix::MatrixRowSlices;
use p3_uni_stark::StarkConfig;
use p3_util::log2_strict_usize;
use rand::distributions::{Distribution, Standard};
use rand::{thread_rng, Rng};
use tracing::info_span;
use tracing_forest::ForestLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

const NUM_STARKS: usize = 8;

/// How many `a * b = c` operations to do per row in the AIR.
const REPETITIONS: usize = 911;
const TRACE_WIDTH: usize = REPETITIONS * 3;
const HEIGHT: usize = 1 << 14;

struct MulAir;

impl<F> BaseAir<F> for MulAir {
    fn width(&self) -> usize { TRACE_WIDTH }
}

impl<AB: AirBuilder> Air<AB> for MulAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let main_local = main.row_slice(0);

        for i in 0..REPETITIONS {
            let start = i * 3;
            let a = main_local[start];
            let b = main_local[start + 1];
            let c = main_local[start + 2];
            builder.assert_zero(a * b - c);
        }
    }
}

fn random_valid_trace<F: Field>(rows: usize) -> RowMajorMatrix<F>
where
    Standard: Distribution<F>, {
    let mut rng = thread_rng();
    let mut trace_values = vec![F::default(); rows * TRACE_WIDTH];
    for (a, b, c) in trace_values.iter_mut().tuples() {
        *a = rng.gen();
        *b = rng.gen();
        *c = *a * *b;
    }
    RowMajorMatrix::new(trace_values, TRACE_WIDTH)
}

/// Note that this is an incomplete prover. Mainly intended for experiment.
/// # Panics
/// This function will panic if the number of traces is not equal to the number
/// of Starks.
pub fn prove<SC: StarkConfig>(config: &SC, mut challenger: SC::Challenger)
where
    Standard: Distribution<SC::Val>, {
    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(ForestLayer::default())
        .init();

    // collect traces of each stark as Matrices
    let traces = [
        random_valid_trace::<SC::Val>(HEIGHT),
        random_valid_trace::<SC::Val>(HEIGHT / 4),
        random_valid_trace::<SC::Val>(HEIGHT / 8),
        random_valid_trace::<SC::Val>(HEIGHT / 8),
        random_valid_trace::<SC::Val>(HEIGHT / 8),
        random_valid_trace::<SC::Val>(HEIGHT / 8),
        random_valid_trace::<SC::Val>(HEIGHT / 8),
        random_valid_trace::<SC::Val>(HEIGHT / 8),
    ];

    // height of each trace matrix
    let degrees: [usize; NUM_STARKS] = traces
        .iter()
        .map(p3_matrix::Matrix::height)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let log_degrees = degrees.map(log2_strict_usize);
    let g_subgroups = log_degrees.map(SC::Val::two_adic_generator);

    // commit to traces
    let (commit, data) = info_span!("commit to trace data")
        .in_scope(|| config.pcs().commit_batches(traces.to_vec()));
    challenger.observe(commit.clone());

    let zeta: SC::Challenge = challenger.sample_ext_element();
    let zeta_and_next: [Vec<SC::Challenge>; NUM_STARKS] =
        core::array::from_fn(|i| vec![zeta, zeta * g_subgroups[i]]);
    challenger.observe(commit.clone());
    let prover_data_and_points = [(&data, zeta_and_next.as_slice())];

    // generate openings proof
    let (_openings, opening_proof) = config
        .pcs()
        .open_multi_batches(&prover_data_and_points, &mut challenger);

    let serialized_proof =
        postcard::to_allocvec(&opening_proof).expect("unable to serialize proof");
    tracing::info!("serialized_proof len: {} bytes", serialized_proof.len());
}

#[cfg(test)]
mod tests {

    use super::prove;
    use crate::config::{DefaultConfig, Mozak3StarkConfig};

    #[test]
    fn test_prove() {
        let (config, challenger) = DefaultConfig::make_config();
        prove(&config, challenger);
    }
}
