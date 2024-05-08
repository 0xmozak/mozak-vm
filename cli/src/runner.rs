//! Utility functions that helps the CLI to interact with the
//! [Mozak runner crate](mozak_runner).
use std::collections::BTreeSet;
use std::io::Read;

use anyhow::Result;
use clio::Input;
use itertools::{izip, Itertools};
use log::debug;
use mozak_runner::elf::Program;
use mozak_runner::state::RawTapes;
use mozak_sdk::common::merkle::merkleize;
use mozak_sdk::common::types::{
    CanonicalOrderedTemporalHints, Poseidon2Hash, ProgramIdentifier, SystemTape,
};
use rkyv::rancor::{Panic, Strategy};
use rkyv::ser::AllocSerializer;

pub fn load_program(mut elf: Input) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");
    Program::mozak_load_program(&elf_bytes)
}

/// Deserializes a serde JSON serialized system tape binary file into a
/// [`SystemTape`].
///
/// # Errors
///
/// Errors if reading from the binary file fails.
pub fn deserialize_system_tape(mut bin: Input) -> Result<SystemTape> {
    let mut sys_tapes_bytes = Vec::new();
    let bytes_read = bin.read_to_end(&mut sys_tapes_bytes)?;
    debug!("Read {bytes_read} of system tape data.");
    let deserialized: SystemTape = serde_json::from_slice(&sys_tapes_bytes)?;
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

pub fn raw_tapes_from_system_tape(sys: &SystemTape, self_prog_id: ProgramIdentifier) -> RawTapes {
    fn serialise<T>(tape: &T, dgb_string: &str) -> Vec<u8>
    where
        T: rkyv::Archive + rkyv::Serialize<Strategy<AllocSerializer<256>, Panic>>, {
        let tape_bytes = rkyv::to_bytes::<_, 256, _>(tape).unwrap().into();
        length_prefixed_bytes(tape_bytes, dgb_string)
    }

    let cast_list = sys
        .call_tape
        .writer
        .iter()
        .map(|msg| msg.callee)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect_vec();

    let canonical_order_temporal_hints: Vec<CanonicalOrderedTemporalHints> = sys
        .event_tape
        .writer
        .get(&self_prog_id)
        .cloned()
        .unwrap_or_default()
        .get_canonical_order_temporal_hints();

    let events_commitment_tape = merkleize(
        canonical_order_temporal_hints
            .iter()
            .map(|x| {
                (
                    // May not be the best idea if
                    // `addr` > goldilock's prime, cc
                    // @Kapil
                    u64::from_le_bytes(x.0.address.inner()),
                    x.0.canonical_hash(),
                )
            })
            .collect::<Vec<(u64, Poseidon2Hash)>>(),
    )
    .0;

    let cast_list_commitment_tape =
        merkleize(izip!(0.., &cast_list).map(|(idx, x)| (idx, x.0)).collect()).0;

    println!("Self Prog ID: {self_prog_id:#?}");
    println!("Found events: {:#?}", canonical_order_temporal_hints.len());

    RawTapes {
        private_tape: length_prefixed_bytes(
            sys.private_input_tape
                .writer
                .get(&self_prog_id)
                .cloned()
                .unwrap_or_default()
                .0,
            "PRIVATE_TAPE",
        ),
        public_tape: length_prefixed_bytes(
            sys.public_input_tape
                .writer
                .get(&self_prog_id)
                .cloned()
                .unwrap_or_default()
                .0,
            "PUBLIC_TAPE",
        ),
        call_tape: serialise(&sys.call_tape.writer, "CALL_TAPE"),
        event_tape: serialise(&canonical_order_temporal_hints, "EVENT_TAPE"),
        self_prog_id_tape: self_prog_id.0 .0,
        events_commitment_tape,
        cast_list_commitment_tape,
    }
}
