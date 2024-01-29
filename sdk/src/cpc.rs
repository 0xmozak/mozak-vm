#[cfg(not(target_os = "zkvm"))]
use std::sync::Mutex;

use crate::coretypes::ProgramIdentifier;

#[cfg(not(target_os = "zkvm"))]
lazy_static::lazy_static! {
    static ref global_transcript_tape: Mutex<Vec<crate::coretypes::CPCMessage>> = Mutex::new(Vec::new());
}

#[allow(unused_variables)]
#[allow(unreachable_code)]
pub fn cross_program_call<T>(program: ProgramIdentifier, method: u8, calldata: &[u8]) -> T
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
pub fn globaltrace_add_message(program: ProgramIdentifier, method: u8, calldata: &[u8]) {
    use crate::coretypes::CPCMessage;
    let msg = CPCMessage {
        recipient_program: program,
        recipient_method: method,
        calldata,
    };

    println!("globaltrace_add_message called for CPC message: {:?}", msg);

    if let Ok(mut guard) = global_transcript_tape.lock() {
        guard.push(msg);
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
