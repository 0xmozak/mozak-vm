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

use crate::columns_view::HasNamedColumns;
use crate::expr::{build_ext, build_packed, ConstraintBuilder, GenerateConstraints};
use crate::storage_device::columns::{StorageDevice, NUM_STORAGE_DEVICE_COLS};
use crate::unstark::NoColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct StorageDeviceStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for StorageDeviceStark<F, D> {
    type Columns = StorageDevice<F>;
    type PublicInputs = NoColumns<F>;
}

const COLUMNS: usize = NUM_STORAGE_DEVICE_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<'a, F, T: Copy, U, const D: usize>
    GenerateConstraints<'a, T, StorageDevice<Expr<'a, T>>, NoColumns<U>>
    for StorageDeviceStark<F, { D }>
{
    // Design description - https://docs.google.com/presentation/d/1J0BJd49BMQh3UR5TrOhe3k67plHxnohFtFVrMpDJ1oc/edit?usp=sharing
    fn generate_constraints(
        vars: &StarkFrameTyped<StorageDevice<Expr<'a, T>>, NoColumns<U>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let nv = vars.next_values;
        let mut constraints = ConstraintBuilder::default();

        constraints.always(lv.ops.is_memory_store.is_binary());
        constraints.always(lv.ops.is_storage_device.is_binary());
        constraints.always(lv.is_executed().is_binary());

        // If nv.is_storage_device() == 1: lv.size == 0, also forces the last row to be
        // size == 0 ! This constraints ensures loop unrolling was done correctly
        constraints.always(nv.ops.is_storage_device * lv.size);
        // If lv.is_lv_and_nv_are_memory_rows == 1:
        //    nv.address == lv.address + 1 (wrapped)
        //    nv.size == lv.size - 1 (not-wrapped)
        let added = lv.addr + 1;
        let wrapped = added - (1 << 32);
        // nv.address == lv.address + 1 (wrapped)
        constraints
            .always(lv.is_lv_and_nv_are_memory_rows * (nv.addr - added) * (nv.addr - wrapped));
        // nv.size == lv.size - 1 (not-wrapped)
        constraints.transition(nv.is_lv_and_nv_are_memory_rows * (nv.size - (lv.size - 1)));
        // Edge cases:
        //  a) - storage_device with size = 0: <-- this case is solved since CTL from
        // CPU        a.1) is_lv_and_nv_are_memory_rows = 0 (no memory rows
        // inserted)  b) - storage_device with size = 1: <-- this case needs to be
        // solved separately        b.1) is_lv_and_nv_are_memory_rows = 0 (only one
        // memory row inserted) To solve case-b:
        // If lv.is_storage_device() == 1 && lv.size != 0:
        //      lv.addr == nv.addr       <-- next row address must be the same !!!
        //      lv.size === nv.size - 1  <-- next row size is decreased
        constraints.transition(lv.ops.is_storage_device * lv.size * (nv.addr - lv.addr));
        constraints.transition(lv.ops.is_storage_device * lv.size * (nv.size - (lv.size - 1)));
        // If lv.is_storage_device() == 1 && lv.size == 0:
        //      nv.is_memory() == 0 <-- next op can be only io - since size == 0
        // This one is ensured by:
        //  1) is_binary(storage_device or memory)
        //  2) if nv.is_storage_device() == 1: lv.size == 0

        // If lv.is_storage_device() == 1 && nv.size != 0:
        //      nv.is_lv_and_nv_are_memory_rows == 1
        constraints
            .always(lv.ops.is_storage_device * nv.size * (nv.is_lv_and_nv_are_memory_rows - 1));

        constraints
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for StorageDeviceStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

        where
            FE: FieldExtension<D2, BaseField = F>,
            P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let eb = ExprBuilder::default();
        let constraints = Self::generate_constraints(&eb.to_typed_starkframe(vars));
        build_packed(constraints, consumer);
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let constraints = Self::generate_constraints(&eb.to_typed_starkframe(vars));
        build_ext(constraints, builder, consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use mozak_runner::code::execute_code_with_ro_memory;
    use mozak_runner::decode::ECALL;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::state::RawTapes;
    use mozak_runner::test_utils::{u32_extra, u8_extra};
    use mozak_sdk::core::constants::DIGEST_BYTES;
    use mozak_sdk::core::ecall::{self};
    use mozak_sdk::core::reg_abi::{REG_A0, REG_A1, REG_A2};
    use plonky2::plonk::config::Poseidon2GoldilocksConfig;
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    use starky::stark_testing::test_stark_circuit_constraints;

    use crate::stark::mozak_stark::MozakStark;
    use crate::storage_device::stark::StorageDeviceStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    pub fn prove_read_private_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::PRIVATE_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RawTapes::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_public_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::PUBLIC_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RawTapes::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_call_tape_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::CALL_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RawTapes::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_event_tape_zero_size<Stark: ProveAndVerify>(address: u32) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::EVENT_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 0),       // A2 - size
            ],
            RawTapes::default(),
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_private<Stark: ProveAndVerify>(address: u32, private_tape: Vec<u8>) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::PRIVATE_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RawTapes {
                private_tape,
                ..Default::default()
            },
        );
        assert_ne!(
            record.last_state.private_tape.data.len(),
            0,
            "Proving an execution with an empty tape might make our tests pass, even if things are wrong"
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_public<Stark: ProveAndVerify>(address: u32, public_tape: Vec<u8>) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::PUBLIC_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RawTapes {
                public_tape,
                ..Default::default()
            },
        );

        assert_ne!(
            record.last_state.public_tape.data.len(),
            0,
            "Proving an execution with an empty tape might make our tests pass, even if things are wrong"
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_call_tape<Stark: ProveAndVerify>(address: u32, call_tape: Vec<u8>) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::CALL_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RawTapes {
                call_tape,
                ..Default::default()
            },
        );
        assert_ne!(
            record.last_state.call_tape.data.len(),
            0,
            "Proving an execution with an empty tape might make our tests pass, even if things are wrong"
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_event_tape<Stark: ProveAndVerify>(address: u32, event_tape: Vec<u8>) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &[(address, 0)],
            &[
                (REG_A0, ecall::EVENT_TAPE),
                (REG_A1, address), // A1 - address
                (REG_A2, 1),       // A2 - size
            ],
            RawTapes {
                event_tape,
                ..Default::default()
            },
        );
        assert_ne!(
            record.last_state.event_tape.data.len(),
            0,
            "Proving an execution with an empty tape might make our tests pass, even if things are wrong"
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_events_commitment_tape<Stark: ProveAndVerify>(
        address: u32,
        events_commitment_tape: [u8; 32],
    ) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &(0..DIGEST_BYTES)
                .map(|i| (address.wrapping_add(u32::try_from(i).unwrap()), 0_u8))
                .collect_vec(),
            &[
                (REG_A0, ecall::EVENTS_COMMITMENT_TAPE),
                (REG_A1, address),                              // A1 - address
                (REG_A2, u32::try_from(DIGEST_BYTES).unwrap()), // A2 - size
            ],
            RawTapes {
                events_commitment_tape,
                ..Default::default()
            },
        );

        assert_ne!(
            record.last_state.events_commitment_tape.len(),
            0,
            "Proving an execution with an empty tape might make our tests pass, even if things are wrong"
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_cast_list_commitment_tape<Stark: ProveAndVerify>(
        address: u32,
        cast_list_commitment_tape: [u8; 32],
    ) {
        let (program, record) = execute_code_with_ro_memory(
            // set sys-call IO_READ in x10(or a0)
            [ECALL],
            &[],
            &(0..DIGEST_BYTES)
                .map(|i| (address.wrapping_add(u32::try_from(i).unwrap()), 0_u8))
                .collect_vec(),
            &[
                (REG_A0, ecall::CAST_LIST_COMMITMENT_TAPE),
                (REG_A1, address),                              // A1 - address
                (REG_A2, u32::try_from(DIGEST_BYTES).unwrap()), // A2 - size
            ],
            RawTapes {
                cast_list_commitment_tape,
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();

        assert_ne!(
            record.last_state.cast_list_commitment_tape.len(),
            0,
            "Proving an execution with an empty tape might make our tests pass, even if things are wrong"
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    pub fn prove_read_explicit<Stark: ProveAndVerify>(address: u32, content: u8) {
        let (program, record) = execute_code_with_ro_memory(
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
                        imm: ecall::PRIVATE_TAPE,
                        ..Args::default()
                    },
                },
                // add ecall to read
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
            &[],
            &[
                (address, 0),
                (address.wrapping_add(1), 0),
                (address.wrapping_add(2), 0),
                (address.wrapping_add(3), 0),
            ],
            &[],
            RawTapes {
                private_tape: vec![content, content, content, content],
                ..Default::default()
            },
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_read_private_zero_size_mozak(address in u32_extra()) {
            prove_read_private_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_read_private_mozak(address in u32_extra(), content in u8_extra()) {
            prove_read_private::<MozakStark<F, D>>(address, vec![content]);
        }
        #[test]
        fn prove_read_public_zero_size_mozak(address in u32_extra()) {
            prove_read_public_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_read_public_mozak(address in u32_extra(), content in u8_extra()) {
            prove_read_public::<MozakStark<F, D>>(address, vec![content]);
        }
        #[test]
        fn prove_read_call_tape_zero_size_mozak(address in u32_extra()) {
            prove_read_call_tape_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_read_call_tape_mozak(address in u32_extra(), content in u8_extra()) {
            prove_read_call_tape::<MozakStark<F, D>>(address, vec![content]);
        }

        #[test]
        fn prove_read_event_tape_zero_size_mozak(address in u32_extra()) {
            prove_read_event_tape_zero_size::<MozakStark<F, D>>(address);
        }
        #[test]
        fn prove_read_event_tape_mozak(address in u32_extra(), content in u8_extra()) {
            prove_read_event_tape::<MozakStark<F, D>>(address, vec![content]);
        }

        #[test]
        fn prove_events_commitment_tape_mozak(address in u32_extra(), content in u8_extra()) {
            prove_events_commitment_tape::<MozakStark<F, D>>(address, [content; 32]);
        }

        #[test]
        fn prove_cast_list_commitment_tape_mozak(address in u32_extra(), content in u8_extra()) {
            prove_cast_list_commitment_tape::<MozakStark<F, D>>(address, [content; 32]);
        }

        #[test]
        fn prove_read_mozak_explicit(address in u32_extra(), content in u8_extra()) {
            prove_read_explicit::<MozakStark<F, D>>(address, content);
        }
    }
    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        type C = Poseidon2GoldilocksConfig;
        type S = StorageDeviceStark<F, D>;

        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}
