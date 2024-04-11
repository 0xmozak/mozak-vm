use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use crate::columns_view::HasNamedColumns;
use crate::memory_io::columns::{InputOutputMemory, NUM_IO_MEM_COLS};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct InputOutputMemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for InputOutputMemoryStark<F, D> {
    type Columns = InputOutputMemory<F>;
}

const COLUMNS: usize = NUM_IO_MEM_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for InputOutputMemoryStark<F, D> {
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
        let lv: &InputOutputMemory<P> = vars.get_local_values().into();
        let nv: &InputOutputMemory<P> = vars.get_next_values().into();

        is_binary(yield_constr, lv.ops.is_memory_store);
        is_binary(yield_constr, lv.ops.is_io_store);
        is_binary(yield_constr, lv.is_executed());

        // If nv.is_io() == 1: lv.size == 0, also forces the last row to be size == 0 !
        // This constraints ensures loop unrolling was done correctly
        yield_constr.constraint(nv.ops.is_io_store * lv.size);
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
            lv.ops.is_io_store * lv.size * (nv.addr - lv.addr),
        );
        yield_constr.constraint_transition(
            lv.ops.is_io_store * lv.size * (nv.size - (lv.size - P::ONES)),
        );
        // If lv.is_io() == 1 && lv.size == 0:
        //      nv.is_memory() == 0 <-- next op can be only io - since size == 0
        // This one is ensured by:
        //  1) is_binary(io or memory)
        //  2) if nv.is_io() == 1: lv.size == 0

        // If lv.is_io() == 1 && nv.size != 0:
        //      nv.is_lv_and_nv_are_memory_rows == 1
        yield_constr.constraint(lv.ops.is_io_store * nv.size * (nv.is_lv_and_nv_are_memory_rows - P::ONES));
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &InputOutputMemory<ExtensionTarget<D>> = vars.get_local_values().into();
        let nv: &InputOutputMemory<ExtensionTarget<D>> = vars.get_next_values().into();

        let is_executed = builder.add_extension(lv.ops.is_memory_store, lv.ops.is_io_store);

        is_binary_ext_circuit(builder, lv.ops.is_memory_store, yield_constr);
        is_binary_ext_circuit(builder, lv.ops.is_io_store, yield_constr);
        is_binary_ext_circuit(builder, is_executed, yield_constr);

        let is_io_mul_lv_size = builder.mul_extension(nv.ops.is_io_store, lv.size);
        yield_constr.constraint(builder, is_io_mul_lv_size);

        let wrap_at = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
        let one = builder.one_extension();
        let added = builder.add_extension(lv.addr, one);
        let wrapped = builder.sub_extension(added, wrap_at);

        let nv_addr_sub_added = builder.sub_extension(nv.addr, added);
        let is_lv_and_nv_are_memory_rows_mul_nv_addr_sub_added =
            builder.mul_extension(lv.is_lv_and_nv_are_memory_rows, nv_addr_sub_added);
        let nv_addr_sub_wrapped = builder.sub_extension(nv.addr, wrapped);
        let constraint = builder.mul_extension(
            is_lv_and_nv_are_memory_rows_mul_nv_addr_sub_added,
            nv_addr_sub_wrapped,
        );
        yield_constr.constraint(builder, constraint);

        let lv_size_sub_one = builder.sub_extension(lv.size, one);
        let nv_size_sub_lv_size_sub_one = builder.sub_extension(nv.size, lv_size_sub_one);
        let constraint =
            builder.mul_extension(nv.is_lv_and_nv_are_memory_rows, nv_size_sub_lv_size_sub_one);
        yield_constr.constraint_transition(builder, constraint);

        let nv_addr_sub_lv_addr = builder.sub_extension(nv.addr, lv.addr);
        let is_io_mul_lv_size = builder.mul_extension(lv.ops.is_io_store, lv.size);
        let constraint = builder.mul_extension(is_io_mul_lv_size, nv_addr_sub_lv_addr);
        yield_constr.constraint_transition(builder, constraint);

        let constraint = builder.mul_extension(is_io_mul_lv_size, nv_size_sub_lv_size_sub_one);
        yield_constr.constraint_transition(builder, constraint);

        let lv_is_io_mul_nv_size = builder.mul_extension(lv.ops.is_io_store, nv.size);
        let is_lv_and_nv_are_memory_rows_sub_one =
            builder.sub_extension(nv.is_lv_and_nv_are_memory_rows, one);
        let constraint =
            builder.mul_extension(lv_is_io_mul_nv_size, is_lv_and_nv_are_memory_rows_sub_one);
        yield_constr.constraint(builder, constraint);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use mozak_runner::decode::ECALL;
    use mozak_runner::elf::RuntimeArguments;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{u32_extra_except_mozak_ro_memory, u8_extra};
    use mozak_runner::util::execute_code_with_runtime_args;
    use mozak_sdk::core::ecall::{self, COMMITMENT_SIZE};
    use mozak_sdk::core::reg_abi::{REG_A0, REG_A1, REG_A2};
    use plonky2::plonk::config::Poseidon2GoldilocksConfig;
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    use starky::stark_testing::test_stark_circuit_constraints;

    use crate::memory_io::stark::InputOutputMemoryStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    pub fn prove_io_read_private_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[(address, 0)],
            &[
                (REG_A0, ecall::IO_READ_PRIVATE),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RuntimeArguments::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_public_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[(address, 0)],
            &[
                (REG_A0, ecall::IO_READ_PUBLIC),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RuntimeArguments::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_call_tape_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[(address, 0)],
            &[
                (REG_A0, ecall::IO_READ_CALL_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RuntimeArguments::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_private<Stark: ProveAndVerify>(address: u32, io_tape_private: Vec<u8>) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[(address, 0)],
            &[
                (REG_A0, ecall::IO_READ_PRIVATE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RuntimeArguments {
                io_tape_private,
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_public<Stark: ProveAndVerify>(address: u32, io_tape_public: Vec<u8>) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[(address, 0)],
            &[
                // TODO: this looks like a bug, it should be IO_READ_PUBLIC?
                (REG_A0, ecall::IO_READ_CALL_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RuntimeArguments {
                io_tape_public,
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_call_tape<Stark: ProveAndVerify>(address: u32, call_tape: Vec<u8>) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[(address, 0)],
            &[
                (REG_A0, ecall::IO_READ_CALL_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RuntimeArguments {
                call_tape,
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_events_commitment_tape<Stark: ProveAndVerify>(
        address: u32,
        events_commitment_tape: Vec<u8>,
    ) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &(0..COMMITMENT_SIZE)
                .map(|i| (address.wrapping_add(u32::try_from(i).unwrap()), 0_u8))
                .collect_vec(),
            &[
                (REG_A0, ecall::EVENTS_COMMITMENT_TAPE),
                (REG_A1, address),                                 // A1 - address
                (REG_A2, u32::try_from(COMMITMENT_SIZE).unwrap()), // A2 - size
            ],
            RuntimeArguments {
                events_commitment_tape,
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_cast_list_commitment_tape<Stark: ProveAndVerify>(
        address: u32,
        cast_list_commitment_tape: Vec<u8>,
    ) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &(0..COMMITMENT_SIZE)
                .map(|i| (address.wrapping_add(u32::try_from(i).unwrap()), 0_u8))
                .collect_vec(),
            &[
                (REG_A0, ecall::CAST_LIST_COMMITMENT_TAPE),
                (REG_A1, address),                        // A1 - address
                (REG_A2, u32::try_from(COMMITMENT_SIZE)), // A2 - size
            ],
            RuntimeArguments {
                cast_list_commitment_tape,
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read<Stark: ProveAndVerify>(address: u32, content: u8) {
        let (program, record) = execute_code_with_runtime_args(
            // set sys-call IO_READ in x10(or a0)
            [
                ECALL,
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A1,
                        imm: address,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A2,
                        imm: 1,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A0,
                        imm: ecall::IO_READ_PUBLIC,
                        ..Args::default()
                    },
                },
                ECALL,
            ],
            &[(address, 0)],
            &[
                (REG_A0, ecall::IO_READ_PRIVATE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RuntimeArguments {
                self_prog_id: vec![content],
                cast_list: vec![content],
                events_commitment_tape: vec![content],
                cast_list_commitment_tape: vec![content],
                io_tape_private: vec![content],
                io_tape_public: vec![content],
                call_tape: vec![content],
                event_tape: vec![content],
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_io_read_explicit<Stark: ProveAndVerify>(address: u32, content: u8) {
        let (program, record) = execute_code_with_runtime_args(
            [
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A1,
                        imm: address,
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
                        imm: ecall::IO_READ_PRIVATE,
                        ..Args::default()
                    },
                },
                // add ecall to io_read
                ECALL,
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
            &[
                (address, 0),
                (address.wrapping_add(1), 0),
                (address.wrapping_add(2), 0),
                (address.wrapping_add(3), 0),
            ],
            &[],
            RuntimeArguments {
                io_tape_private: vec![content, content, content, content],
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_io_read_private_zero_size_mozak(address in u32_extra_except_mozak_ro_memory()) {
            prove_io_read_private_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_io_read_private_mozak(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_io_read_private::<MozakStark<F, D>>(address, vec![content]);
        }
        #[test]
        fn prove_io_read_public_zero_size_mozak(address in u32_extra_except_mozak_ro_memory()) {
            prove_io_read_public_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_io_read_public_mozak(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_io_read_public::<MozakStark<F, D>>(address, vec![content]);
        }
        #[test]
        fn prove_io_read_call_tape_zero_size_mozak(address in u32_extra_except_mozak_ro_memory()) {
            prove_io_read_call_tape_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_io_read_call_tape_mozak(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_io_read_call_tape::<MozakStark<F, D>>(address, vec![content]);
        }

        #[test]
        fn prove_events_commitment_tape_mozak(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_events_commitment_tape::<MozakStark<F, D>>(address, vec![content]);
        }

        #[test]
        fn prove_cast_list_commitment_tape_mozak(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_cast_list_commitment_tape::<MozakStark<F, D>>(address, vec![content]);
        }

        #[test]
        fn prove_io_read_mozak(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_io_read::<MozakStark<F, D>>(address, content);
        }
        #[test]
        fn prove_io_read_mozak_explicit(address in u32_extra_except_mozak_ro_memory(), content in u8_extra()) {
            prove_io_read_explicit::<MozakStark<F, D>>(address, content);
        }
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        type C = Poseidon2GoldilocksConfig;
        type S = InputOutputMemoryStark<F, D>;

        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
