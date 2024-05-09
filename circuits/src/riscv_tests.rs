#[cfg(test)]
mod tests {
    use anyhow::{Ok, Result};
    use mozak_runner::elf::Program;
    use mozak_runner::state::State;
    use mozak_runner::vm::step;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2_maybe_rayon::*;
    use starky::config::StarkConfig;

    use crate::test_utils::prove_and_verify_mozak_stark;

    /// This function takes the contents of a compiled ELF and runs it through
    /// the Mozak VM runner to ensure correctness of the base RISC-V
    /// implementation. Afterwards, we prove and verify the execution.
    ///
    /// Below, we use a set of test files compiled from <https://github.com/riscv-software-src/riscv-tests>.
    /// specifically the rv32ui and rv32um tests.
    ///
    /// These files are generated on the first `cargo build` using Docker which
    /// downloads the RISC-V toolchain and compiles these test files into ELFs.
    ///
    /// To use these tests, this function specifically asserts that the value of
    /// x10 == 0 at the end of a run, as defined by `RVTEST_PASS` here: <https://github.com/riscv/riscv-test-env/blob/4fabfb4e0d3eacc1dc791da70e342e4b68ea7e46/p/riscv_test.h#L247-L252>
    /// Custom tests may be added as long as the assertion is respected.
    fn run_test(elf: &[u8]) -> Result<()> {
        let _ = env_logger::try_init();
        let program = Program::vanilla_load_elf(elf)?;
        let state = State::<GoldilocksField>::from(program.clone());
        let record = step(&program, state)?;
        let state = record.last_state.clone();
        // At the end of every test,
        // register a0(x10) is set to 0 before an ECALL if it passes
        assert_eq!(state.get_register_value(10), 0);
        assert_eq!(state.get_register_value(17), 93);
        assert!(state.has_halted());

        let config = StarkConfig::standard_fast_config();
        prove_and_verify_mozak_stark(&program, &record, &config)?;
        Ok(())
    }

    #[test]
    fn riscv_tests() {
        mozak_examples::riscv_tests.into_par_iter().for_each(|elf| {
            run_test(elf).unwrap();
        });
    }
}
