    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::prover::prove;
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use mozak_circuits::poseidon2::generation::generate_poseidon2_trace;
    use mozak_circuits::poseidon2::stark::Poseidon2_12Stark;
    use mozak_circuits::stark::utils::trace_rows_to_poly_values;
    // use mozak_circuits::test_utils::{create_poseidon2_test, Poseidon2Test};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2_12Stark<F, D>;

    // fn poseidon2_constraints() -> Result<()> {
    //     let mut config = StarkConfig::standard_fast_config();
    //     config.fri_config.cap_height = 0;
    //     config.fri_config.rate_bits = 3; // to meet the constraint degree bound

    //     let (_program, record) = create_poseidon2_test(&[Poseidon2Test {
    //         data: "😇 Mozak is knowledge arguments based technology".to_string(),
    //         input_start_addr: 1024,
    //         output_start_addr: 2048,
    //     }]);

    //     let step_rows = record.executed;

    //     let stark = S::default();
    //     let trace = generate_poseidon2_trace(&step_rows);
    //     let trace_poly_values = trace_rows_to_poly_values(trace);

    //     let proof = prove::<F, C, S, D>(
    //         stark,
    //         &config,
    //         trace_poly_values,
    //         &[],
    //         &mut TimingTree::default(),
    //     )?;
    //     verify_stark_proof(stark, proof, &config)
    // }

    fn poseidon2_stark_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    // fn test_circuit() -> anyhow::Result<()> {
    //     let stark = S::default();
    //     test_stark_circuit_constraints::<F, C, S, D>(stark)?;
    //     Ok(())
    // }
    
    fn main() -> Result<(), anyhow::Error> {
        // poseidon2_constraints()?;
        poseidon2_stark_degree()?;
        // test_circuit()?;
        Ok(())
    }