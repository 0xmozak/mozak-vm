use std::marker::PhantomData;

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
use crate::memory::columns::{is_executed_ext_circuit, Memory};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

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

    // Constraints design: https://docs.google.com/presentation/d/1G4tmGl8V1W0Wqxv-MwjGjaM3zUF99dzTvFhpiood4x4/edit?usp=sharing
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        // TODO(Matthias): add a constraint to forbid two is_init in a row (with the
        // same address).  See `circuits/src/generation/memoryinit.rs` in
        // `a75c8fbc2701a4a6b791b2ff71857795860c5591`
        let lv: &Memory<P> = vars.get_local_values().into();
        let nv: &Memory<P> = vars.get_next_values().into();

        // Boolean constraints
        // -------------------
        // Constrain certain columns of the memory table to be only
        // boolean values.
        is_binary(yield_constr, lv.is_writable);
        is_binary(yield_constr, lv.is_store);
        is_binary(yield_constr, lv.is_load);
        is_binary(yield_constr, lv.is_init);
        is_binary(yield_constr, lv.is_executed());

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
        let zero_init_clk = P::ONES - lv.clk;
        let elf_init_clk = lv.clk;

        // first row init is always one or its a dummy row
        yield_constr.constraint_first_row((P::ONES - lv.is_init) * lv.is_executed());

        // All init ops happen prior to exec and the `clk` would be `0` or `1`.
        yield_constr.constraint(lv.is_init * zero_init_clk * elf_init_clk);
        // All zero inits should have value `0`.
        // (Assumption: `is_init` == 1, `clk` == 0)
        yield_constr.constraint(lv.is_init * zero_init_clk * lv.value);
        // All zero inits should be writable.
        // (Assumption: `is_init` == 1, `clk` == 0)
        yield_constr.constraint(lv.is_init * zero_init_clk * (P::ONES - lv.is_writable));

        // Operation constraints
        // ---------------------
        // No `SB` operation can be seen if memory address is not marked `writable`
        yield_constr.constraint((P::ONES - lv.is_writable) * lv.is_store);

        // For all "load" operations, the value cannot change between rows
        yield_constr.constraint(nv.is_load * (nv.value - lv.value));

        // Clock constraints
        // -----------------
        // `diff_clk` assumes the value "new row's `clk`" - "current row's `clk`" in
        // case both new row and current row talk about the same addr. However,
        // in case the "new row" describes an `addr` different from the current
        // row, we expect `diff_clk` to be `0`. New row's clk remains
        // unconstrained in such situation.
        yield_constr
            .constraint_transition((P::ONES - nv.is_init) * (nv.diff_clk - (nv.clk - lv.clk)));
        yield_constr.constraint_transition(lv.is_init * lv.diff_clk);

        // Padding constraints
        // -------------------
        // Once we have padding, all subsequent rows are padding; ie not
        // `is_executed`.
        yield_constr
            .constraint_transition((lv.is_executed() - nv.is_executed()) * nv.is_executed());

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
        yield_constr.constraint_transition(diff_addr * (P::ONES - diff_addr * nv.diff_addr_inv));

        // b) checking that nv.is_init == 1 only when nv.diff_addr_inv != 0
        // Note: nv.diff_addr_inv != 0 IFF: lv.addr != nv.addr
        yield_constr.constraint_transition(diff_addr * nv.diff_addr_inv - nv.is_init);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &Memory<ExtensionTarget<D>> = vars.get_local_values().into();
        let nv: &Memory<ExtensionTarget<D>> = vars.get_next_values().into();

        is_binary_ext_circuit(builder, lv.is_writable, yield_constr);
        is_binary_ext_circuit(builder, lv.is_store, yield_constr);
        is_binary_ext_circuit(builder, lv.is_load, yield_constr);
        is_binary_ext_circuit(builder, lv.is_init, yield_constr);
        let lv_is_executed = is_executed_ext_circuit(builder, lv);
        is_binary_ext_circuit(builder, lv_is_executed, yield_constr);

        let one = builder.one_extension();
        let one_minus_is_init = builder.sub_extension(one, lv.is_init);
        let one_minus_is_init_times_executed =
            builder.mul_extension(one_minus_is_init, lv_is_executed);
        yield_constr.constraint_first_row(builder, one_minus_is_init_times_executed);

        let one_sub_clk = builder.sub_extension(one, lv.clk);
        let is_init_mul_one_sub_clk = builder.mul_extension(lv.is_init, one_sub_clk);

        let is_init_mul_one_sub_clk_mul_clk =
            builder.mul_extension(is_init_mul_one_sub_clk, lv.clk);
        yield_constr.constraint(builder, is_init_mul_one_sub_clk_mul_clk);

        let is_init_mul_clk_mul_value = builder.mul_extension(is_init_mul_one_sub_clk, lv.value);
        yield_constr.constraint(builder, is_init_mul_clk_mul_value);

        let one_sub_is_writable = builder.sub_extension(one, lv.is_writable);
        let is_init_mul_clk_mul_one_sub_is_writable =
            builder.mul_extension(is_init_mul_one_sub_clk, one_sub_is_writable);
        yield_constr.constraint(builder, is_init_mul_clk_mul_one_sub_is_writable);

        let is_store_mul_one_sub_is_writable =
            builder.mul_extension(lv.is_store, one_sub_is_writable);
        yield_constr.constraint(builder, is_store_mul_one_sub_is_writable);

        let nv_value_sub_lv_value = builder.sub_extension(nv.value, lv.value);
        let is_load_mul_nv_value_sub_lv_value =
            builder.mul_extension(nv.is_load, nv_value_sub_lv_value);
        yield_constr.constraint(builder, is_load_mul_nv_value_sub_lv_value);

        let one_sub_nv_is_init = builder.sub_extension(one, nv.is_init);
        let nv_clk_sub_lv_clk = builder.sub_extension(nv.clk, lv.clk);
        let nv_diff_clk_sub_nv_clk_sub_lv_clk =
            builder.sub_extension(nv.diff_clk, nv_clk_sub_lv_clk);
        let one_sub_nv_is_init_mul_nv_diff_clk_sub_nv_clk_sub_lv_clk =
            builder.mul_extension(one_sub_nv_is_init, nv_diff_clk_sub_nv_clk_sub_lv_clk);
        yield_constr.constraint_transition(
            builder,
            one_sub_nv_is_init_mul_nv_diff_clk_sub_nv_clk_sub_lv_clk,
        );
        let lv_is_init_mul_lv_diff_clk = builder.mul_extension(lv.is_init, lv.diff_clk);
        yield_constr.constraint_transition(builder, lv_is_init_mul_lv_diff_clk);

        let nv_is_executed = is_executed_ext_circuit(builder, nv);
        let lv_is_executed_sub_nv_is_executed =
            builder.sub_extension(lv_is_executed, nv_is_executed);
        let constr = builder.mul_extension(nv_is_executed, lv_is_executed_sub_nv_is_executed);
        yield_constr.constraint_transition(builder, constr);

        let diff_addr = builder.sub_extension(nv.addr, lv.addr);
        let diff_addr_mul_diff_addr_inv = builder.mul_extension(diff_addr, nv.diff_addr_inv);
        let one_sub_diff_addr_mul_diff_addr_inv =
            builder.sub_extension(one, diff_addr_mul_diff_addr_inv);
        let diff_addr_one_sub_diff_addr_mul_diff_addr_inv =
            builder.mul_extension(diff_addr, one_sub_diff_addr_mul_diff_addr_inv);
        yield_constr.constraint_transition(builder, diff_addr_one_sub_diff_addr_mul_diff_addr_inv);

        let diff_addr_mul_diff_addr_inv_sub_nv_is_init =
            builder.sub_extension(diff_addr_mul_diff_addr_inv, nv.is_init);
        yield_constr.constraint_transition(builder, diff_addr_mul_diff_addr_inv_sub_nv_is_init);
    }
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
    use crate::cross_table_lookup::CrossTableLookup;
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
    use crate::generation::poseidon2_output_bytes::generate_poseidon2_output_bytes_trace;
    use crate::generation::poseidon2_sponge::generate_poseidon2_sponge_trace;
    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::stark::mozak_stark::{MozakStark, TableKind, TableKindSetBuilder};
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
        let poseiden2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);
        #[allow(unused)]
        let poseidon2_output_bytes_rows =
            generate_poseidon2_output_bytes_trace(&poseiden2_sponge_rows);
        let mut memory_rows = generate_memory_trace(
            &record.executed,
            &generate_memory_init_trace(&program),
            &halfword_memory_rows,
            &fullword_memory_rows,
            &io_memory_private_rows,
            &io_memory_public_rows,
            &poseiden2_sponge_rows,
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
        let ctl = CrossTableLookup::new(
            vec![
                memoryinit::columns::lookup_for_memory(TableKind::ElfMemoryInit),
                memoryinit::columns::lookup_for_memory(TableKind::MozakMemoryInit),
                memory_zeroinit::columns::lookup_for_memory(),
            ],
            memory::columns::lookup_for_memoryinit(),
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
        assert!(check_single_ctl::<F>(&trace, &ctl).is_err());
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
