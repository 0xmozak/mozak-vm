pub use mozak_vm::elf::Code;
use mozak_vm::elf::Program;

use crate::rpc::message::Argument;
use crate::space::object::Object;
use crate::ApplicationStorage;

/// Executes the VM instance on the provided program and returns the output of
/// the program as well as updated states.
///
/// We do not support programs that make calls to other programs yet.
/// Though we can recursively call this function to support that.
#[allow(unused_variables)] // TODO - remove
pub fn run_program(
    elf: &Code,
    inputs: &Vec<Argument>,
    memory: &ApplicationStorage,
) -> (Vec<Argument>, Vec<Object>, Vec<Object>) {
    // Execute the VM instance here and return the updated state
    // We will need to convert from the Message Input to the VM Input format

    let program = Program {
        entry_point: 0,
        ro_memory: Default::default(),
        rw_memory: Default::default(),
        ro_code: elf.clone(),
    };

    return (vec![], vec![], vec![]);
}
