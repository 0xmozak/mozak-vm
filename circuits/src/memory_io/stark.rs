use std::fmt::Display;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use crate::memory_io::columns::{InputOutputMemory, NUM_IO_MEM_COLS};
use crate::stark::utils::is_binary;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct InputOuputMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> Display for InputOuputMemoryStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InputOutputMemoryStark")
    }
}

const COLUMNS: usize = NUM_IO_MEM_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for InputOuputMemoryStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

        where
            FE: FieldExtension<D2, BaseField = F>,
            P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    // Design description - https://docs.google.com/presentation/d/1J0BJd49BMQh3UR5TrOhe3k67plHxnohFtFVrMpDJ1oc/edit?usp=sharing
    #[rustfmt::skip]
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &InputOutputMemory<P> = vars.get_local_values().try_into().unwrap();
        let nv: &InputOutputMemory<P> = vars.get_next_values().try_into().unwrap();

        is_binary(yield_constr, lv.ops.is_memory_store);
        is_binary(yield_constr, lv.ops.is_io_store);
        is_binary(yield_constr, lv.is_executed());

        // If nv.is_io() == 1: lv.size == 0, also forces the last row to be size == 0 !
        // This constraints ensures loop unrolling was done correctly  
        yield_constr.constraint(nv.is_io() * lv.size);
        // If lv.is_lv_and_nv_are_memory_rows == 1:
        //    nv.address == lv.address + 1 (wrapped)
        //    nv.size == lv.size - 1 (not-wrapped)
        let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
        let added = lv.addr + P::ONES;
        let wrapped = added - wrap_at;
        // nv.address == lv.address + 1 (wrapped)
        yield_constr
            .constraint(lv.is_lv_and_nv_are_memory_rows * (nv.addr - added) * (nv.addr - wrapped));
        // nv.size == lv.size - 1 (not-wrapped)
        yield_constr.constraint_transition(
            nv.is_lv_and_nv_are_memory_rows * (nv.size - (lv.size - P::ONES)),
        );
        // Edge cases:
        //  a) - io_store with size = 0: <-- this case is solved since CTL from CPU 
        //        a.1) is_lv_and_nv_are_memory_rows = 0 (no memory rows inserted) 
        //  b) - io_store with size = 1: <-- this case needs to be solved separately
        //        b.1) is_lv_and_nv_are_memory_rows = 0 (only one memory row inserted) 
        // To solve case-b:
        // If lv.is_io() == 1 && lv.size != 0: 
        //      lv.addr == nv.addr       <-- next row address must be the same !!! 
        //      lv.size === nv.size - 1  <-- next row size is decreased  
        yield_constr.constraint_transition(
            lv.is_io() * lv.size * (nv.addr - lv.addr),
        );
        yield_constr.constraint_transition(
            lv.is_io() * lv.size * (nv.size - (lv.size - P::ONES)),
        );
        // If lv.is_io() == 1 && lv.size == 0: 
        //      nv.is_memory() == 0 <-- next op can be only io - since size == 0
        // This one is ensured by:
        //  1) is_binary(io or memory) 
        //  2) if nv.is_io() == 1: lv.size == 0 
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }

    fn constraint_degree(&self) -> usize { 3 }
}
#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::system::ecall;
    use mozak_runner::system::reg_abi::{REG_A0, REG_A1, REG_A2};
    use mozak_runner::test_utils::{simple_test_code_with_io_tape, u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    pub fn prove_io_read_zero_size<Stark: ProveAndVerify>(offset: u32, imm: u32) {
        let (program, record) = simple_test_code_with_io_tape(
            &[
                // set sys-call IO_READ in x10(or a0)
                Instruction {
                    op: Op::ECALL,
                    args: Args {
                        rd: REG_A0,
                        ..Args::default()
                    },
                },
            ],
            &[(imm.wrapping_add(offset), 0)],
            &[
                (REG_A0, ecall::IO_READ),
                (REG_A1, imm.wrapping_add(offset)), // A1 - address
                (REG_A2, 0),                        // A2 - size
            ],
            &[],
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u8) {
        let (program, record) = simple_test_code_with_io_tape(
            &[
                // set sys-call IO_READ in x10(or a0)
                Instruction {
                    op: Op::ECALL,
                    args: Args {
                        rd: REG_A0,
                        ..Args::default()
                    },
                },
            ],
            &[(imm.wrapping_add(offset), 0)],
            &[
                (REG_A0, ecall::IO_READ),
                (REG_A1, imm.wrapping_add(offset)), // A1 - address
                (REG_A2, 1),                        // A2 - size
            ],
            &[content],
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_explicit<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u8) {
        let (program, record) = simple_test_code_with_io_tape(
            &[
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A1,
                        imm: imm.wrapping_add(offset),
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A2,
                        imm: 4,
                        ..Args::default()
                    },
                },
                // set sys-call IO_READ in x10(or a0)
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A0,
                        imm: ecall::IO_READ,
                        ..Args::default()
                    },
                },
                // add ecall to io_read
                Instruction {
                    op: Op::ECALL,
                    args: Args {
                        rd: REG_A0, // return size
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A0,
                        imm: 0,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A1,
                        imm: 0,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A2,
                        imm: 0,
                        ..Args::default()
                    },
                },
            ],
            &[(imm.wrapping_add(offset), 0)],
            &[],
            &[content, content, content, content],
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_io_read_zero_size_mozak(offset in u32_extra(), imm in u32_extra()) {
            prove_io_read_zero_size::<MozakStark<F, D>>(offset, imm);
        }
        #[test]
        fn prove_io_read_mozak(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_io_read::<MozakStark<F, D>>(offset, imm, content);
        }
        #[test]
        fn prove_io_read_mozak_explicit(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_io_read_explicit::<MozakStark<F, D>>(offset, imm, content);
        }
    }
}
