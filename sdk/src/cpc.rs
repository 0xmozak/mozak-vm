use crate::coretypes::ProgramIdentifier;

#[allow(unused_variables)]
pub fn cross_program_call<T>(program: &ProgramIdentifier, method: u8, calldata: &[u8]) -> T
where
    T: Sized {
    unimplemented!();
}
