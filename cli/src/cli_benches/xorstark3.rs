use mozak_circuits3::config::{DefaultConfig, Mozak3StarkConfig};
use mozak_circuits3::generation::xor::generate_dummy_xor_trace;
use mozak_circuits3::xor::stark::XorStark;
use p3_uni_stark::{prove, verify};

pub fn xor_stark_plonky3(n: u32) -> Result<(), anyhow::Error> {
    let (config, mut challenger) = DefaultConfig::make_config();
    let mut verifer_challenger = challenger.clone();
    let trace = generate_dummy_xor_trace(n);
    let proof = prove::<<DefaultConfig as Mozak3StarkConfig>::MyConfig, _>(
        &config,
        &XorStark,
        &mut challenger,
        trace,
    );

    if verify(&config, &XorStark, &mut verifer_challenger, &proof).is_err() {
        return Err(anyhow::anyhow!("Verification failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_xor_stark_plonky3() { super::xor_stark_plonky3(10).unwrap() }

    #[test]
    fn test_xor_stark_plonky3_run() {
        let bench = BenchArgs {
            function: BenchFunction::XorStark3 { n: 10 },
        };
        bench.run().unwrap();
    }
}
