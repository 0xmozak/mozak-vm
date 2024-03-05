//! Utility functions that helps the CLI to interact with the
//! [Mozak runner crate](mozak_runner).
use std::collections::BTreeSet;
use std::io::Read;

use anyhow::Result;
use clio::Input;
use itertools::Itertools;
use log::debug;
use mozak_runner::elf::Program;
use mozak_sdk::coretypes::{Event, ProgramIdentifier};
use mozak_sdk::sys::SystemTapes;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::Deserialize;

pub fn load_program(mut elf: Input, args: &mozak_runner::elf::RuntimeArguments) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");
    Program::mozak_load_program(&elf_bytes, args)
}

/// Deserializes an rkyv-serialized system tape binary file into `SystemTapes`.
///
/// # Errors
///
/// Errors if reading from the binary file fails.
///
/// # Panics
///
/// Panics if deserialization fails.
pub fn deserialize_system_tape(mut bin: Input) -> Result<SystemTapes> {
    let mut sys_tapes_bytes = Vec::new();
    let bytes_read = bin.read_to_end(&mut sys_tapes_bytes)?;
    debug!("Read {bytes_read} of system tape data.");
    let sys_tapes_archived = unsafe { rkyv::archived_root::<SystemTapes>(&sys_tapes_bytes[..]) };
    let deserialized: SystemTapes = sys_tapes_archived
        .deserialize(&mut rkyv::Infallible)
        .unwrap();
    Ok(deserialized)
}

fn length_prefixed_bytes(data: Vec<u8>, dgb_string: &str) -> Vec<u8> {
    let data_len = data.len();
    let mut len_prefix_bytes = Vec::with_capacity(data_len + 4);
    len_prefix_bytes.extend_from_slice(
        &(u32::try_from(data.len()))
            .expect("length of data's max size shouldn't be more than u32")
            .to_le_bytes(),
    );
    len_prefix_bytes.extend(data);
    debug!(
        "Length-Prefixed {:<15} of byte len: {:>5}, on-mem bytes: {:>5}",
        dgb_string,
        data_len,
        len_prefix_bytes.len()
    );
    len_prefix_bytes
}

/// Deserializes an rkyv-serialized system tape binary file into
/// [`SystemTapes`](mozak_sdk::sys::SystemTapes).
///
/// # Panics
///
/// Panics if conversion from rkyv-serialized system tape to
/// [`RuntimeArguments`](mozak_runner::elf::RuntimeArguments)
/// fails.
pub fn tapes_to_runtime_arguments(
    tape_bin: Input,
    self_prog_id: Option<String>,
) -> mozak_runner::elf::RuntimeArguments {
    let sys_tapes: SystemTapes = deserialize_system_tape(tape_bin).unwrap();
    let self_prog_id: ProgramIdentifier = self_prog_id.unwrap_or_default().into();

    let cast_list = sys_tapes
        .call_tape
        .writer
        .iter()
        .flat_map(|msg| [msg.caller_prog, msg.callee_prog])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect_vec();

    let event_tape_single: Vec<Event> = sys_tapes
        .event_tape
        .writer
        .into_iter()
        .find_map(|t| (t.id == self_prog_id).then_some(t.contents))
        .unwrap_or_default();

    debug!("Self Prog ID: {self_prog_id:#?}");
    debug!("Cast List (canonical repr): {cast_list:#?}");

    {
        fn serialise<T>(tape: &T, dgb_string: &str) -> Vec<u8>
        where
            T: rkyv::Archive + rkyv::Serialize<AllocSerializer<256>>, {
            let tape_bytes = rkyv::to_bytes::<_, 256>(tape).unwrap().into();
            length_prefixed_bytes(tape_bytes, dgb_string)
        }

        mozak_runner::elf::RuntimeArguments {
            self_prog_id: self_prog_id.to_le_bytes().to_vec(),
            cast_list: serialise(&cast_list, "CAST_LIST"),
            io_tape_public: serialise(&sys_tapes.public_tape, "IO_TAPE_PUBLIC"),
            io_tape_private: serialise(&sys_tapes.private_tape, "IO_TAPE_PRIVATE"),
            call_tape: serialise(&sys_tapes.call_tape.writer, "CALL_TAPE"),
            event_tape: serialise(&event_tape_single, "EVENT_TAPE"),
        }
    }
}
