use std::marker::PhantomData;

use expr::{Expr, ExprBuilder};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, Constraint, ConstraintBuilder};
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

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for MemoryStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let eb = ExprBuilder::default();
        let lv: &Memory<_> = vars.get_local_values().into();
        let lv = lv.map(|x| eb.lit(x));
        let nv: &Memory<_> = vars.get_next_values().into();
        let nv = nv.map(|x| eb.lit(x));
        let cb = generate_constraints(&eb, lv, nv).into();
        build_packed(cb, yield_constr);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let lv: &Memory<_> = vars.get_local_values().into();
        let lv = lv.map(|x| eb.lit(x));
        let nv: &Memory<_> = vars.get_next_values().into();
        let nv = nv.map(|x| eb.lit(x));
        let cb = generate_constraints(&eb, lv, nv).into();
        build_ext(cb, circuit_builder, yield_constr);
    }
}

// A Script to generate constraints
// Constraints design: https://docs.google.com/presentation/d/1G4tmGl8V1W0Wqxv-MwjGjaM3zUF99dzTvFhpiood4x4/edit?usp=sharing
fn generate_constraints<'a, V>(
    eb: &'a ExprBuilder,
    lv: Memory<Expr<'a, V>>,
    nv: Memory<Expr<'a, V>>,
) -> Vec<Constraint<Expr<'a, V>>>
where
    V: Copy, {
    // TODO(Matthias): add a constraint to forbid two is_init in a row (with the
    // same address).  See `circuits/src/generation/memoryinit.rs` in
    // `a75c8fbc2701a4a6b791b2ff71857795860c5591`
    let one: Expr<'_, V> = eb.one();
    let mut cb = ConstraintBuilder::default();

    // Memory initialization Constraints
    // ---------------------------------
    // The memory table is assumed to be ordered by `addr` in ascending order.
    // such that whenever we describe an memory init / access
    // pattern of an "address", a correct table guarantees the following:
    //
    // All rows for a specific `addr` MUST start with one, or both, of:
    //   1) a zero init (case for heap / other dynamic addresses).
    //   2) a memory init via static ELF (hereby referred to as elf init), or
    // For these starting rows, `is_init` will be true.
    //
    // 1) Zero Init
    //   All zero initialized memory will have clk `0` and value `0`. They
    //   should also be writable.
    //
    // 2) ELF Init
    //   All elf init rows will have clk `1`.
    //
    // In principle, zero initializations for a certain address MUST come
    // before any elf initializations to ensure we don't zero out any memory
    // initialized by the ELF. This is constrained via a rangecheck on `diff_clk`.
    // Since clk is in ascending order, any memory address with a zero init
    // (`clk` == 0) after an elf init (`clk` == 1) would be caught by
    // this range check.
    //
    // Note that if `diff_clk` range check is removed, we must
    // include a new constraint that constrains the above relationship.
    //
    // NOTE: We rely on 'Ascending ordered, contiguous
    // "address" view constraint' to provide us with a guarantee of single
    // contiguous block of rows per `addr`. If that guarantee does not exist,
    // for some address `x`, different contiguous blocks of rows in memory
    // table can present case for them being derived from static ELF and
    // dynamic (execution) at the same time or being writable as
    // well as non-writable at the same time.
    //
    // A zero init at clk == 0,
    // while an ELF init happens at clk == 1.
    let zero_init_clk = one - lv.clk;
    let elf_init_clk = lv.clk;

    // Boolean constraints
    // -------------------
    // Constrain certain columns of the memory table to be only
    // boolean values.
    cb.constraint(eb.is_binary(lv.is_writable));
    cb.constraint(eb.is_binary(lv.is_store));
    cb.constraint(eb.is_binary(lv.is_load));
    cb.constraint(eb.is_binary(lv.is_init));
    cb.constraint(eb.is_binary(lv.is_executed()));

    // first row init is always one or its a dummy row
    cb.constraint_first_row((one - lv.is_init) * lv.is_executed());

    // All init ops happen prior to exec and the `clk` would be `0` or `1`.
    cb.constraint(lv.is_init * zero_init_clk * elf_init_clk);
    // All zero inits should have value `0`.
    // (Assumption: `is_init` == 1, `clk` == 0)
    cb.constraint(lv.is_init * zero_init_clk * lv.value);
    // All zero inits should be writable.
    // (Assumption: `is_init` == 1, `clk` == 0)
    cb.constraint(lv.is_init * zero_init_clk * (one - lv.is_writable));

    // Operation constraints
    // ---------------------
    // No `SB` operation can be seen if memory address is not marked `writable`
    cb.constraint((one - lv.is_writable) * lv.is_store);

    // For all "load" operations, the value cannot change between rows
    cb.constraint(nv.is_load * (nv.value - lv.value));

    // Clock constraints
    // -----------------
    // `diff_clk` assumes the value "new row's `clk`" - "current row's `clk`" in
    // case both new row and current row talk about the same addr. However,
    // in case the "new row" describes an `addr` different from the current
    // row, we expect `diff_clk` to be `0`. New row's clk remains
    // unconstrained in such situation.
    cb.constraint_transition((one - nv.is_init) * (nv.diff_clk - (nv.clk - lv.clk)));
    cb.constraint_transition(lv.is_init * lv.diff_clk);

    // Padding constraints
    // -------------------
    // Once we have padding, all subsequent rows are padding; ie not
    // `is_executed`.
    cb.constraint_transition((lv.is_executed() - nv.is_executed()) * nv.is_executed());

    // We can have init == 1 row only when address is changing. More specifically,
    // is_init has to be the first row in an address block.
    // a) checking diff-addr-inv was computed correctly
    // If next.address - current.address == 0
    // --> next.diff_addr_inv = 0
    // Else current.address - next.address != 0
    //  --> next.diff_addr_inv != 0 but (lv.addr - nv.addr) * nv.diff_addr_inv == 1
    //  --> so, expression: (P::ONES - (lv.addr - nv.addr) * nv.diff_addr_inv) == 0
    // NOTE: we don't really have to constrain diff-addr-inv to be zero when address
    // does not change at all, so, this constrain can be removed, and the
    // `diff_addr * nv.diff_addr_inv - nv.is_init` constrain will be enough to
    // ensure that diff-addr-inv for the case of address change was indeed computed
    // correctly. We still prefer to leave this code, because maybe diff-addr-inv
    // can be usefull for feature scenarios, BUT if we will want to take advantage
    // on last 0.001% of perf, it can be removed (if other parts of the code will
    // not use it somewhere)
    // TODO(Roman): how we insure sorted addresses - via RangeCheck:
    // MemoryTable::new(Column::singles_diff([col_map().addr]), mem.is_executed())
    // Please add test that fails with not-sorted memory trace
    let diff_addr = nv.addr - lv.addr;
    cb.constraint_transition(diff_addr * (one - diff_addr * nv.diff_addr_inv));

    // b) checking that nv.is_init == 1 only when nv.diff_addr_inv != 0
    // Note: nv.diff_addr_inv != 0 IFF: lv.addr != nv.addr
    cb.constraint_transition((diff_addr * nv.diff_addr_inv) - nv.is_init);

    cb.collect()
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::util::execute_code;
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
        let (program, record) = execute_code(instructions, &[], &[(1, iterations)]);
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
        let instructions = [Instruction {
            op: Op::SB,
            args: Args {
                rs1: 1,
                rs2: 1,
                imm: 0,
                ..Args::default()
            },
        }];
        let (program, record) = execute_code(instructions, &[(0, 0)], &[(1, 0)]);
        let memory_init_rows = generate_elf_memory_init_trace(&program);
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
            &generate_memory_init_trace(&program),
            &halfword_memory_rows,
            &fullword_memory_rows,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &poseidon2_sponge_rows,
            &poseidon2_output_bytes_rows,
        );
        // malicious prover sets first memory row's is_init to zero
        memory_rows[0].is_init = F::ZERO;
        // fakes a load instead of init
        memory_rows[0].is_load = F::ONE;
        // now all addresses are same, and all inits are zero
        assert!(memory_rows
            .iter()
            .all(|row| row.is_init == F::ZERO && row.addr == F::ZERO));

        let memory_zeroinit_rows =
            generate_memory_zero_init_trace::<F>(&memory_init_rows, &record.executed, &program);

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
            elf_memory_init_stark: trace_rows_to_poly_values(memory_init_rows),
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
