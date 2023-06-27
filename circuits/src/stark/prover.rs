use anyhow::Result;
use mozak_vm::vm::Row;
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::config::Hasher;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::prover::prove as prove_table;
use starky::stark::Stark;

use super::mozak_stark::{MozakStark, NUM_TABLES};
use super::proof::AllProof;
use crate::cpu::stark::CpuStark;
use crate::generation::generate_traces;

#[allow(clippy::missing_errors_doc)]
pub fn prove<F, C, const D: usize>(
    step_rows: &[Row],
    mozak_stark: &mut MozakStark<F, D>,
    config: &StarkConfig,
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    let trace_poly_values = generate_traces(step_rows);
    prove_with_traces(mozak_stark, config, &trace_poly_values, timing)
}

#[allow(clippy::missing_errors_doc)]
pub fn prove_with_traces<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    trace_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    let cpu_proof = prove_table(
        mozak_stark.cpu_stark,
        config,
        trace_poly_values[0].clone(),
        [],
        timing,
    )?;
    let stark_proofs = [cpu_proof];

    Ok(AllProof { stark_proofs })
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test, simple_test_code};

    use crate::test_utils::simple_proof_test;

    #[test]
    fn prove_halt() {
        let record = simple_test(0, &[], &[]);
        simple_proof_test(&record.executed).unwrap();
    }

    #[test]
    fn prove_lui() {
        let record = simple_test(4, &[(0_u32, 0x8000_00b7 /* lui r1, 0x80000 */)], &[]);
        assert_eq!(record.last_state.get_register_value(1), 0x8000_0000);
        simple_proof_test(&record.executed).unwrap();
    }

    #[test]
    fn prove_lui_2() {
        let record = simple_test_code(
            &[Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 1,
                    imm: 0xDEAD_BEEF,
                    ..Args::default()
                },
            }],
            &[],
            &[],
        );
        assert_eq!(record.last_state.get_register_value(1), 0xDEAD_BEEF,);
        simple_proof_test(&record.executed).unwrap();
    }

    #[test]
    #[should_panic]
    fn prove_beq() {
        let record = simple_test_code(
            &[Instruction {
                op: Op::BEQ,
                args: Args {
                    rs1: 0,
                    rs2: 1,
                    imm: 42,
                    ..Args::default()
                },
            }],
            &[],
            &[(1, 2)],
        );
        assert_eq!(record.last_state.get_pc(), 8);
        simple_proof_test(&record.executed).unwrap();
    }
}
