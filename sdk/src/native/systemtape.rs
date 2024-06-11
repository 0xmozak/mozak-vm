use std::collections::BTreeSet;
use std::fs;

use rkyv::rancor::*;
use rkyv::ser::AllocSerializer;
use rkyv::Deserialize;

use crate::common::traits::CallArgument;
use crate::common::types::{
    CanonicalEvent, CanonicalOrderedTemporalHints, CrossProgramCall, ProgramIdentifier,
};

/// Writes a byte slice to a given file
fn write_to_file(file_path: &str, content: &[u8]) {
    use std::io::Write;
    let path = std::path::Path::new(file_path);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(content).unwrap();
}

fn length_prefixed_bytes(data: Vec<u8>) -> Vec<u8> {
    let data_len = data.len();
    let mut len_prefix_bytes = Vec::with_capacity(data_len + 4);
    len_prefix_bytes.extend_from_slice(
        &(u32::try_from(data.len()))
            .expect("length of data's max size shouldn't be more than u32")
            .to_le_bytes(),
    );
    len_prefix_bytes.extend(data);
    len_prefix_bytes
}
fn serialise<T>(tape: &T) -> Vec<u8>
where
    T: rkyv::Archive + rkyv::Serialize<Strategy<AllocSerializer<256>, Panic>>, {
    let tape_bytes = rkyv::to_bytes::<_, 256, _>(tape).unwrap().into();
    length_prefixed_bytes(tape_bytes)
}

/// Dumps a copy of `SYSTEM_TAPE` to disk, serialized
/// via `serde_json` as well as in rust debug file format
/// if opted for. Extension of `.tape.json` is used for serialized
/// formed of tape on disk, `.tape.debug` will be used for
/// debug tape on disk.
#[allow(dead_code)]
fn dump_system_tape(is_debug_tape_required: bool) {
    fs::create_dir_all("out").unwrap();
    let tape_clone = unsafe {
        crate::common::system::SYSTEM_TAPE.clone() // .clone() removes `Lazy{}`
    };

    if is_debug_tape_required {
        write_to_file("out/tape.debug", &format!("{tape_clone:#?}").into_bytes());
    }

    println!("{:?}", tape_clone.call_tape);
    let ser = serialise(&tape_clone.call_tape.writer);
    let archived_cpc_messages = rkyv::access::<Vec<CrossProgramCall>, Panic>(&ser).unwrap();
    let len = archived_cpc_messages.len();
    let cast_list: Vec<ProgramIdentifier> = archived_cpc_messages
        .iter()
        .map(|m| {
            m.callee
                .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                .unwrap()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    println!("cast list = {:?}", cast_list);

    for msg in archived_cpc_messages.iter() {
        let callee: ProgramIdentifier = msg
            .callee
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();
        let caller: ProgramIdentifier = msg
            .caller
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();
        let args = rkyv::from_bytes::<(), Failure>(&msg.argument.0).unwrap();
        let ret = rkyv::from_bytes::<(), Failure>(&msg.return_.0).unwrap();

        println!("Callee = {:?}", callee);
        println!("Caller = {:?}", caller);
        println!("args = {:?} ret = {:?}", args, ret);
    }

    println!("{:?}", archived_cpc_messages);
    let self_prog_id = ProgramIdentifier::from(String::from(
        "MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538",
    ));
    println!("Self prog id {:?}", self_prog_id);
    let canonical_order_temporal_hints: Vec<CanonicalOrderedTemporalHints> = tape_clone
        .event_tape
        .writer
        .get(&self_prog_id)
        .cloned()
        .unwrap_or_default()
        .get_canonical_order_temporal_hints();

    let events = serialise(&canonical_order_temporal_hints);
    let asd = rkyv::access::<Vec<CanonicalOrderedTemporalHints>, Panic>(&events).unwrap();

    // for i in asd.iter() {
    //     let canonical_event: CanonicalEvent =
    //         i.deserialize(Strategy::<_, Panic>::wrap(&mut ())).unwrap();
    // }

    println!("{:?}", asd);
    write_to_file(
        "out/tape.json",
        &serde_json::to_string_pretty(&tape_clone)
            .unwrap()
            .into_bytes(),
    );
}

/// This functions dumps 2 files of the currently running guest program:
///   1. the actual system tape (JSON),
///   2. the debug dump of the system tape,
///
/// These are all dumped in a sub-directory named `out` in the project root. The
/// user must be cautious to not move at least the system tape, as the system
/// tape is used by the CLI in proving and in transaction bundling, and the SDK
/// makes some assumptions about where to find the ELF for proving.
pub fn dump_proving_files() { dump_system_tape(true); }
