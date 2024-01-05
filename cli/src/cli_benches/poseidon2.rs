use std::iter::repeat_with;

use mozak_circuits::test_utils::{
    create_poseidon2_test, prove_and_verify_mozak_stark, Poseidon2Test,
};
use starky::config::StarkConfig;

#[allow(clippy::pedantic)]
pub fn poseidon2_bench(input_len: u32) -> Result<(), anyhow::Error> {
    let s: String = repeat_with(fastrand::alphanumeric)
        .take(input_len as usize)
        .collect();
    let (program, record) = create_poseidon2_test(&[Poseidon2Test {
        data: s,
        input_start_addr: 1024,
        output_start_addr: 1024 + input_len,
    }]);

    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_poseidon2_bench() { super::poseidon2_bench(10).unwrap(); }

    #[test]
    fn test_poseidon2_bench_with_run() {
        let function = BenchFunction::Poseidon2Bench { input_len: 10 };
        let bench = BenchArgs { function };
        bench.run().unwrap();
    }
}
