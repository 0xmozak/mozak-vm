use std::io::Write;
#[cfg(not(target_os = "zkvm"))]
use std::sync::Mutex;

use rkyv::{Archive, Deserialize, Serialize};

use crate::coretypes::ProgramIdentifier;

#[cfg(not(target_os = "zkvm"))]
lazy_static::lazy_static! {
    static ref global_transcript_tape: Mutex<Vec<crate::coretypes::CPCMessage>> = Mutex::new(Vec::new());
}

#[allow(unused_variables)]
#[allow(clippy::needless_pass_by_value)]
#[allow(unreachable_code)]
#[must_use]
pub fn cross_program_call<T>(program: ProgramIdentifier, method: u8, calldata: Vec<u8>) -> T
where
    T: Sized + Default, {
    #[cfg(not(target_os = "zkvm"))]
    {
        globaltrace_add_message(program, method, calldata);
        return T::default();
    }
    unimplemented!();
}

#[cfg(not(target_os = "zkvm"))]
pub fn globaltrace_add_message(program: ProgramIdentifier, method: u8, calldata: Vec<u8>) {
    use crate::coretypes::CPCMessage;
    let msg = CPCMessage {
        recipient_program: program,
        recipient_method: method,
        calldata,
    };

    println!("globaltrace_add_message called for CPC message: {:?}", msg);

    if let Ok(mut guard) = global_transcript_tape.lock() {
        // Serializing is as easy as a single function call
        let bytes = rkyv::to_bytes::<_, 256>(&msg).unwrap();

        guard.push(msg);

        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("transcript")
            .expect("cannot open file");

        out.write(&bytes).expect("write failed");
    } else {
        // Handle the case where the lock is poisoned
        panic!("Failed to acquire lock on global_transcript_tape");
    }
}

// #[cfg(not(target_os = "zkvm"))]
// pub fn globaltrace_dump_to_disk<T>(file: std::path::Path)
// {
//     // TODO
// }
