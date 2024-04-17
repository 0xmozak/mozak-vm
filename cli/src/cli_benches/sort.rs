use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_examples::MOZAK_SORT_ELF;
use mozak_runner::elf::Program;
use mozak_runner::state::{RawTapes, State};
use mozak_runner::vm::step;
use starky::config::StarkConfig;

pub fn sort_bench(n: u32) -> Result<(), anyhow::Error> {
    let program = Program::vanilla_load_elf(MOZAK_SORT_ELF)?;
    let raw_tapes = RawTapes {
        public_tape: n.to_le_bytes().to_vec(),
        ..Default::default()
    };
    let state = State::new(program.clone(), raw_tapes);
    let record = step(&program, state)?;

    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_sort_bench_with_run() {
        let n = 10;
        let bench = BenchArgs {
            function: BenchFunction::SortBench { n },
        };
        bench.run().unwrap();
    }
}
