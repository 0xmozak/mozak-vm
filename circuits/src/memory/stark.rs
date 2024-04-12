use std::marker::PhantomData;

use expr::{Expr, ExprBuilder, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, ConstraintBuilder};
use crate::memory::columns::Memory;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for MemoryStark<F, D> {
    type Columns = Memory<F>;
}

const COLUMNS: usize = Memory::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

fn generate_constraints<'a, T: Copy, U, const N2: usize>(
    vars: &StarkFrameTyped<Memory<Expr<'a, T>>, [U; N2]>,
) -> ConstraintBuilder<Expr<'a, T>> {
    let lv = vars.local_values;
    let nv = vars.next_values;
    let mut constraints = ConstraintBuilder::default();

    // Boolean constraints
    // -------------------
    // Constrain certain columns of the memory table to be only
    // boolean values.
    for selector in [
        lv.is_writable,
        lv.is_store,
        lv.is_load,
        lv.is_init,
        lv.is_executed(),
    ] {
        constraints.always(selector.is_binary());
    }

    // Address constraints
    // -------------------

    // We start address at 0 and end at u32::MAX
    // This saves us a rangecheck on the address,
    // but we rangecheck the address difference.
    constraints.first_row(lv.addr);
    constraints.last_row(lv.addr - i64::from(u32::MAX));

    // Address can only change for init in the new row...
    constraints.always((1 - nv.is_init) * (nv.addr - lv.addr));
    // ... and we have a range-check to make sure that addresses go up for each
    // init.

    // Dummy also needs to have the same address as rows before _and_ after; apart
    // from the last dummy in the trace.
    constraints.transition((1 - lv.is_executed()) * (nv.addr - lv.addr));

    // Writable constraints
    // --------------------

    // writeable only changes for init:
    constraints.always((1 - nv.is_init) * (nv.is_writable - lv.is_writable));

    // No `SB` operation can be seen if memory address is not marked `writable`
    constraints.always((1 - lv.is_writable) * lv.is_store);

    // Value constraint
    // -----------------
    // Only init and store can change the value.  Dummy and read stay the same.
    constraints.always((nv.is_init + nv.is_store - 1) * (nv.value - lv.value));

    constraints
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for MemoryStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let eb = ExprBuilder::default();
        let constraints = generate_constraints(&eb.to_typed_starkframe(vars));
        build_packed(constraints, consumer);
    }

    fn eval_ext_circuit(
        &self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let constraints = generate_constraints(&eb.to_typed_starkframe(vars));
        build_ext(constraints, circuit_builder, consumer);
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove;
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use crate::cross_table_lookup::ctl_utils::check_single_ctl;
    use crate::cross_table_lookup::CrossTableLookupWithTypedOutput;
    use crate::generation::fullword_memory::generate_fullword_memory_trace;
    use crate::generation::halfword_memory::generate_halfword_memory_trace;
    use crate::generation::io_memory::{
        generate_io_memory_private_trace, generate_io_memory_public_trace,
    };
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
    use crate::generation::memoryinit::{
        generate_elf_memory_init_trace, generate_memory_init_trace,
        generate_mozak_memory_init_trace,
    };
    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
    use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
    use crate::stark::mozak_stark::{
        ElfMemoryInitTable, MozakMemoryInitTable, MozakStark, TableKindSetBuilder,
    };
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{fast_test_config, ProveAndVerify};
    use crate::{memory, memory_zeroinit, memoryinit};

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = MemoryStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_memory_sb_lb_all() -> Result<()> {
        let (program, executed) = memory_trace_test_case(1);
        MozakStark::prove_and_verify(&program, &executed)?;
        Ok(())
    }

    #[test]
    fn prove_memory_sb_lb() -> Result<()> {
        for repeats in 0..8 {
            let (program, executed) = memory_trace_test_case(repeats);
            MemoryStark::prove_and_verify(&program, &executed)?;
        }
        Ok(())
    }

    pub fn memory<Stark: ProveAndVerify>(
        iterations: u32,
        addr_offset: u32,
    ) -> Result<(), anyhow::Error> {
        let instructions = [
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 1,
                    rs1: 1,
                    imm: 1_u32.wrapping_neg(),
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SB,
                args: Args {
                    rs1: 1,
                    rs2: 1,
                    imm: addr_offset,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::BLT,
                args: Args {
                    rs1: 0,
                    rs2: 1,
                    imm: 0,
                    ..Args::default()
                },
            },
        ];
        let (program, record) = code::execute(instructions, &[], &[(1, iterations)]);
        Stark::prove_and_verify(&program, &record)
    }

    /// If all addresses are equal in memorytable, then setting all `is_init` to
    /// zero should fail.
    ///
    /// Note this is required since this time, `diff_addr_inv` logic  can't help
    /// detect `is_init` for first row.
    ///
    /// This will panic, if debug assertions are enabled in plonky2. So we need
    /// to have two different versions of `should_panic`; see below.
    #[test]
    // This will panic, if debug assertions are enabled in plonky2.
    #[cfg_attr(debug_assertions, should_panic = "Constraint failed in")]
    fn no_init_fail() {
        type F = GoldilocksField;
        let instructions = [Instruction {
            op: Op::SB,
            args: Args {
                rs1: 1,
                rs2: 1,
                imm: 1,
                ..Args::default()
            },
        }];
        let (program, record) = code::execute(instructions, &[(0, 0)], &[(1, 0)]);

        let memory_init = generate_memory_init_trace(&program);
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, &program);

        let elf_memory_init_rows = generate_elf_memory_init_trace(&program);
        let mozak_memory_init_rows = generate_mozak_memory_init_trace(&program);

        let halfword_memory_rows = generate_halfword_memory_trace(&record.executed);
        let fullword_memory_rows = generate_fullword_memory_trace(&record.executed);
        let io_memory_private_rows = generate_io_memory_private_trace(&record.executed);
        let io_memory_public_rows = generate_io_memory_public_trace(&record.executed);
        let poseidon2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);
        #[allow(unused)]
        let poseidon2_output_bytes_rows =
            generate_poseidon2_output_bytes_trace(&poseidon2_sponge_rows);
        let mut memory_rows = generate_memory_trace(
            &record.executed,
            &memory_init,
            &memory_zeroinit_rows,
            &halfword_memory_rows,
            &fullword_memory_rows,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &poseidon2_sponge_rows,
            &poseidon2_output_bytes_rows,
        );
        // malicious prover sets first memory row's is_init to zero
        memory_rows[1].is_init = F::ZERO;
        // fakes a load instead of init
        memory_rows[1].is_load = F::ONE;
        // now address 1 no longer has an init.
        assert!(memory_rows
            .iter()
            .all(|row| row.addr != F::ONE || row.is_init == F::ZERO));

        // ctl for is_init values
        let ctl = CrossTableLookupWithTypedOutput::new(
            vec![
                memoryinit::columns::lookup_for_memory(ElfMemoryInitTable::new),
                memoryinit::columns::lookup_for_memory(MozakMemoryInitTable::new),
                memory_zeroinit::columns::lookup_for_memory(),
            ],
            vec![memory::columns::lookup_for_memoryinit()],
        );

        let memory_trace = trace_rows_to_poly_values(memory_rows);
        let trace = TableKindSetBuilder {
            memory_stark: memory_trace.clone(),
            elf_memory_init_stark: trace_rows_to_poly_values(elf_memory_init_rows),
            memory_zeroinit_stark: trace_rows_to_poly_values(memory_zeroinit_rows),
            mozak_memory_init_stark: trace_rows_to_poly_values(mozak_memory_init_rows),
            ..Default::default()
        }
        .build();

        let config = fast_test_config();

        let stark = S::default();

        // ctl for malicious prover indeed fails, showing inconsistency in is_init
        assert!(check_single_ctl::<F>(&trace, &ctl.to_untyped_output()).is_err());
        let proof = prove::<F, C, S, D>(
            stark,
            &config,
            memory_trace,
            &[],
            &mut TimingTree::default(),
        )
        .unwrap();
        // so memory stark proof should fail too.
        assert!(verify_stark_proof(stark, proof, &config).is_err());
    }

    #[test]
    fn prove_memory_mozak_example() { memory::<MozakStark<F, D>>(150, 0).unwrap(); }

    use mozak_runner::test_utils::{u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_memory_mozak(iterations in u8_extra(), addr_offset in u32_extra()) {
            memory::<MozakStark<F, D>>(iterations.into(), addr_offset).unwrap();
        }
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
