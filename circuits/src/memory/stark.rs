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
use crate::memory::columns::Memory;
use crate::stark::utils::is_binary;

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
        let lv: &Memory<P> = vars.get_local_values().into();
        let nv: &Memory<P> = vars.get_next_values().into();

        // Boolean variables describing whether the current row and
        // next row has a change of address when compared to the
        // previous entry in the table. This works on the assumption
        // that any change in addr will have non-zero `diff_addr` and
        // consequently `diff_addr_inv` values. Any non-zero values for
        // `diff_addr` will lead "correct" `diff_addr_inv` to be multiplicative
        // inverse in the field leading multiplied value `1`. In case there is
        // no change in addr, `diff_addr` (and consequently `diff_addr_inv`)
        // remain `0` when multiplied to each other give `0`.
        let (is_local_a_new_addr, is_next_a_new_addr) = (
            lv.diff_addr * lv.diff_addr_inv, // constrained below
            nv.diff_addr * nv.diff_addr_inv, // constrained below
        );

        // Boolean constraints
        // -------------------
        // Constrain certain columns of the memory table to be only
        // boolean values.
        is_binary(yield_constr, lv.is_writable);
        is_binary(yield_constr, lv.is_store);
        is_binary(yield_constr, lv.is_load);
        is_binary(yield_constr, lv.is_init);
        is_binary(yield_constr, lv.is_executed());

        // `is_local_a_new_addr` should be binary. To keep constraint degree <= 3,
        // the following is used
        yield_constr.constraint(lv.diff_addr * (P::ONES - is_local_a_new_addr));

        // `is_next_a_new_addr` should be binary. However under context where `nv` is
        // `lv` a similar test runs as given above constraining it being a
        // boolean. Hence, we do not explicitly check for `is_next_a_new_addr`
        // to be boolean here.

        // First row constraints
        // ---------------------
        // When starting off, the first `addr` we encounter is supposed to be
        // relatively away from `0` by `diff_addr`, consequently `addr` and
        // `diff_addr` are same for the first row. As a matter of preference,
        // we can have any `clk` in the first row, but `diff_clk` is `0`.
        // This is because when `addr` changes, `diff_clk` is expected to be `0`.
        yield_constr.constraint_first_row(lv.diff_addr - lv.addr);
        yield_constr.constraint_first_row(lv.diff_clk);

        // Ascending ordered, contigous "address" view constraint
        // ------------------------------------------------------
        // All memory init / accesses for a given `addr` is described via contigous
        // rows. This is constrained by range-check on `diff_addr` which in 32-bit
        // RISC can only assume values 0..1<<32. If similar range-checking
        // constraint is put on `addr` as well, the only possibility of
        // non-contigous address view occurs when the prime order of field in
        // question is of size less than 2*(2^32 - 1).

        // Memory initialization Constraints
        // ---------------------------------
        // The memory table is assumed to be ordered by `addr` in ascending order.
        // such that whenever we describe an memory init / access
        // pattern of an "address", a correct table gurantees the following:
        //    All rows for a specific `addr` start with either a memory init (via static
        //    ELF) with `is_init` flag set (case for ro or rw static memory) or `SB`
        //    (case for heap / other dynamic addresses). It is assumed that static
        //    memory init operation happens before any execution has started and
        //    consequently `clk` should be `0` for such entries.
        // NOTE: We rely on 'Ascending ordered, contigous "address" view constraint'
        // to provide us with a guarantee of single contigous block of rows per `addr`.
        // If that gurantee does not exist, for some address `x`, different contigous
        // blocks of rows in memory table can present case for them being derived from
        // static ELF and dynamic (execution) at the same time or being writable as
        // well as non-writable at the same time.

        // All memory init happens prior to exec and the `clk` would be `0`.
        yield_constr.constraint(lv.is_init * lv.clk);

        // If instead, the `addr` talks about an address not coming from static ELF,
        // it needs to begin with a `SB` (store) operation before any further access
        // However `clk` value `0` is a special case.
        yield_constr.constraint(lv.diff_addr * lv.clk * (P::ONES - lv.is_store));

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
        yield_constr.constraint_transition(
            (P::ONES - is_next_a_new_addr) * (nv.diff_clk - (nv.clk - lv.clk)),
        );
        yield_constr.constraint_transition(is_local_a_new_addr * lv.diff_clk);

        // Address constraints
        // -------------------
        // We need to ensure that `diff_addr` always encapsulates difference in addr
        // between two rows
        yield_constr.constraint_transition(nv.diff_addr - (nv.addr - lv.addr));

        // Padding constraints
        // -------------------
        // Once we have padding, all subsequent rows are padding; ie not
        // `is_executed`.
        yield_constr
            .constraint_transition((lv.is_executed() - nv.is_executed()) * nv.is_executed());
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::test_stark_low_degree;

    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

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
        let instructions = &[
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
        let (program, record) = simple_test_code(instructions, &[], &[(1, iterations)]);
        Stark::prove_and_verify(&program, &record)
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
}
