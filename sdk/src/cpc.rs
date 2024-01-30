#[cfg(not(target_os = "zkvm"))]
use std::sync::Mutex;

use crate::coretypes::{ProgramIdentifier, RawMessage};

#[cfg(not(target_os = "zkvm"))]
lazy_static::lazy_static! {
    static ref global_transcript_tape: Mutex<Vec<crate::coretypes::CPCMessage>> = Mutex::new(Vec::new());
}

#[allow(unused_variables)]
#[allow(clippy::needless_pass_by_value)]
#[allow(unreachable_code)]
#[must_use]
pub fn cross_program_call<T>(program: ProgramIdentifier, method: u8, calldata: RawMessage) -> T
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
pub fn globaltrace_add_message(program: ProgramIdentifier, method: u8, calldata: RawMessage) {
    use crate::coretypes::{CPCMessage, RawMessage};
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

#[cfg(not(target_os = "zkvm"))]
pub fn globaltrace_dump_to_disk(file_template: String) {
    fn write_to_file(file_path: String, content: &[u8]) {
        use std::io::Write;
        let path = std::path::Path::new(file_path.as_str());
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
    }

    if let Ok(mut guard) = global_transcript_tape.lock() {
        // let raw_pointer: *const crate::coretypes::CPCMessage = guard.as_ptr();
        // let serialized_bytes = rkyv::to_bytes::<_, 4096>(&guard).unwrap();
        // write_to_file(file_template + ".bin", rkyv::to_bytes::<_,
        // 4096>(&guard).unwrap());
        write_to_file(
            file_template + ".debug",
            &format!("{:#?}", guard).into_bytes(),
        );
    } else {
        // Handle the case where the lock is poisoned
        panic!("Failed to acquire lock on global_transcript_tape");
    }
}
