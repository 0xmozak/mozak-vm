use crate::coretypes::ProgramIdentifier;

#[cfg(not(target_os = "zkvm"))]
lazy_static::lazy_static!{
    global_transcript_tape: Vec<CPCMessage> = vec!{};
}

#[allow(unused_variables)]
pub fn cross_program_call<T>(program: &ProgramIdentifier, method: u8, calldata: Vec<u8>) -> T
where
    T: Sized + Default
{
    #[cfg(not(target_os = "zkvm"))]
    {
        globaltrace_add_message(program, method, calldata);
        return;
    }
    unimplemented!();
}

#[cfg(not(target_os = "zkvm"))]
pub fn globaltrace_add_message<T>(program: &ProgramIdentifier, method: u8, calldata: Vec<u8>) -> T
where
    T: Sized + Default 
{
    use crate::coretypes::CPCMessage;

    global_transcript_tape
        .append(CPCMessage{
            recipient_program: program,
            recipient_method: method,
            calldata,
        });
}

#[cfg(not(target_os = "zkvm"))]
pub fn globaltrace_dump_to_disk<T>(file: std::path::Path) 
{
    // TODO
}
