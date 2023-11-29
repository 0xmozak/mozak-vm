use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use mozak_circuits::test_utils::fast_test_config;
use mozak_circuits::xor::columns::{XorColumnsView, XorView};
use mozak_circuits::xor::stark::XorStark;
use plonky2::field::types::Field;
use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
use plonky2::util::timing::TimingTree;
use starky::prover::prove;
use starky::verifier::verify_stark_proof;

pub type S = XorStark<F, D>;
pub const D: usize = 2;
pub type C = Poseidon2GoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;

fn prove_and_verify_stark(trace: Vec<XorColumnsView<F>>) -> Result<(), anyhow::Error> {
    let config = fast_test_config();
    let trace_poly_values = trace_rows_to_poly_values(trace);
    let stark = XorStark::<F, 2>::default();
    let proof = prove::<F, C, S, D>(
        stark,
        &config,
        trace_poly_values,
        &[],
        &mut TimingTree::default(),
    )?;

    verify_stark_proof(stark, proof, &config)
}

fn to_bits(n: u32) -> [u32; 32] {
    let mut bits = [0; 32];
    for i in 0..32 {
        bits[i] = (n >> i) & 1;
    }
    bits
}

fn gen_xor_trace<F: Field>(n: u32) -> Vec<XorColumnsView<F>> {
    let mut trace = vec![];
    for i in 0..n {
        trace.push(
            XorColumnsView {
                is_execution_row: 1,
                execution: XorView {
                    a: i,
                    b: i.wrapping_add(1),
                    out: i ^ (i.wrapping_add(1)),
                },
                limbs: XorView {
                    a: to_bits(i),
                    b: to_bits(i.wrapping_add(1)),
                    out: to_bits(i ^ (i.wrapping_add(1))),
                },
            }
            .map(F::from_canonical_u32),
        );
    }
    let next_power_two = n.next_power_of_two();
    trace.resize(next_power_two as usize, XorColumnsView::default());
    trace
}

pub fn xor_stark_plonky2(n: u32) -> Result<(), anyhow::Error> {
    let trace = gen_xor_trace::<F>(n);
    prove_and_verify_stark(trace)
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_xor_stark_plonky2() { super::xor_stark_plonky2(10).unwrap(); }

    #[test]
    fn test_xor_stark_plonky2_run() {
        let bench = BenchArgs {
            function: BenchFunction::XorStark2 { n: 10 },
        };
        bench.run().unwrap();
    }
}
