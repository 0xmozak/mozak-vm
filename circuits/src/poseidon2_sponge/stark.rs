use core::fmt::Debug;
use std::marker::PhantomData;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::{Poseidon2, Poseidon2Permutation};

use super::columns::NUM_POSEIDON2_SPONGE_COLS;
use crate::columns_view::HasNamedColumns;
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::unstark::NoColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2SpongeConstraints<F> {
    _f: PhantomData<F>,
}

pub type Poseidon2SpongeStark<F, const D: usize> =
    StarkFrom<F, Poseidon2SpongeConstraints<F>, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

impl<F, const D: usize> HasNamedColumns for Poseidon2SpongeStark<F, D> {
    type Columns = Poseidon2Sponge<F>;
}

const COLUMNS: usize = NUM_POSEIDON2_SPONGE_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: Poseidon2> GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }>
    for Poseidon2SpongeConstraints<F>
{
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = Poseidon2Sponge<E>;

    // For design check https://docs.google.com/presentation/d/10Dv00xL3uggWTPc0L91cgu_dWUzhM7l1EQ5uDEI_cjg/edit?usp=sharing
    fn generate_constraints<'a, T: Copy + Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let rate = Poseidon2Permutation::<F>::RATE;
        let state_size = Poseidon2Permutation::<F>::WIDTH;
        // NOTE: clk and address will be used for CTL to CPU for is_init_permute rows
        // only, and not be used for permute rows.
        // For all non dummy rows we have CTL to Poseidon2 permute stark, with preimage
        // and output columns.

        let rate = u8::try_from(rate).expect("rate > 255");
        let state_size = u8::try_from(state_size).expect("state_size > 255");
        let rate_scalar = i64::from(rate);
        let lv = vars.local_values;
        let nv = vars.next_values;
        let mut constraints = ConstraintBuilder::default();

        for val in [lv.ops.is_permute, lv.ops.is_init_permute, lv.gen_output] {
            constraints.always(val.is_binary());
        }
        let is_exe = lv.ops.is_init_permute + lv.ops.is_permute;
        constraints.always(is_exe.is_binary());

        let is_dummy = 1 - is_exe;

        // dummy row does not generate output
        constraints.always(is_dummy * lv.gen_output);

        // if row generates output then it must be last rate sized
        // chunk of input.
        constraints.always(lv.gen_output * (lv.input_len - rate_scalar));

        let is_init_or_dummy = |vars: &Poseidon2Sponge<Expr<'a, T>>| {
            (1 - vars.ops.is_init_permute) * (vars.ops.is_init_permute + vars.ops.is_permute)
        };

        // First row must be init permute or dummy row.
        constraints.first_row(is_init_or_dummy(&lv));
        // if row generates output then next row can be dummy or start of next hashing
        constraints.always(lv.gen_output * is_init_or_dummy(&nv));

        // Clk should not change within a sponge
        constraints.transition(nv.ops.is_permute * (lv.clk - nv.clk));

        let not_last_sponge = (1 - lv.gen_output) * (lv.ops.is_permute + lv.ops.is_init_permute);
        // if current row consumes input and its not last sponge then next row must have
        // length decreases by RATE, note that only actual execution row can consume
        // input
        constraints.transition(not_last_sponge * (lv.input_len - (nv.input_len + rate_scalar)));
        // and input_addr increases by RATE
        constraints.transition(not_last_sponge * (lv.input_addr - (nv.input_addr - rate_scalar)));

        // For each init_permute capacity bits are zero.
        for i in rate..state_size {
            constraints.always(lv.ops.is_init_permute * (lv.preimage[i as usize] - 0));
        }

        // For each permute capacity bits are copied from previous output.
        for i in rate..state_size {
            constraints.always(
                (1 - nv.ops.is_init_permute)
                    * nv.ops.is_permute
                    * (nv.preimage[i as usize] - lv.output[i as usize]),
            );
        }
        constraints
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

    use super::Poseidon2SpongeStark;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{create_poseidon2_test, Poseidon2Test};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2SpongeStark<F, D>;

    fn poseidon2_sponge_constraints(tests: &[Poseidon2Test]) -> Result<()> {
        let _ = env_logger::try_init();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;
        config.fri_config.rate_bits = 3; // to meet the constraint degree bound

        let (_program, record) = create_poseidon2_test(tests);

        let step_rows = record.executed;

        let stark = S::default();
        let trace = generate_poseidon2_sponge_trace(&step_rows);
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
    fn prove_poseidon2_sponge() {
        assert!(poseidon2_sponge_constraints(&[Poseidon2Test {
            data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
            input_start_addr: 1024,
            output_start_addr: 2048,
        }])
        .is_ok());
    }
    #[test]
    fn prove_poseidon2_sponge_multiple() {
        assert!(poseidon2_sponge_constraints(&[
            Poseidon2Test {
                data: "💥 Mozak-VM Rocks With Poseidon2".to_string(),
                input_start_addr: 512,
                output_start_addr: 1024,
            },
            Poseidon2Test {
                data: "😇 Mozak is knowledge arguments based technology".to_string(),
                input_start_addr: 1024 + 32, /* make sure input and output do not overlap with
                                              * earlier call */
                output_start_addr: 2048,
            },
        ])
        .is_ok());
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
