use crate::rpc::message::Argument;
use crate::{Blob, SpaceStorage};

/// Executes the VM instance on the provided program and returns the output of
/// the program as well as updated states.
///
/// We do not support programs that make calls to other programs yet.
/// Though we can recursively call this function to support that.
#[allow(unused_variables)] // TODO - remove
pub fn run_program(
    elf: &ELF,
    inputs: &Vec<Argument>,
    memory: &SpaceStorage,
) -> (Vec<Argument>, Vec<Blob>, Vec<Blob>) {
    // Execute the VM instance here and return the updated state
    // We will need to convert from the Message Input to the VM Input format

    unimplemented!()
}

/// ELF data.
/// TODO - replace with representation from the `mozak-vm` crate.
#[derive(Debug, Clone)]
pub struct ELF {
    /// The entry point of the program.
    pub(crate) entry_point: u64,
    /// The size of the program.
    pub(crate) size: u64,
    /// The code of the program.
    pub(crate) code: Vec<u8>,
}

impl From<&ELF> for Vec<u8> {
    fn from(elf: &ELF) -> Self {
        let entry_point_bytes = elf.entry_point.to_be_bytes().to_vec();
        let size_bytes = elf.size.to_be_bytes().to_vec();
        entry_point_bytes
            .into_iter()
            .chain(size_bytes.into_iter())
            .chain(elf.code.clone().into_iter())
            .collect()
    }
}
