#![feature(restricted_std)]
extern crate alloc;
// use alloc::vec::Vec;
use mozak_sdk::coretypes::{ProgramIdentifier, Signature, StateObject};
use mozak_sdk::sys::event_emit;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum Operation {
    TransferTo(ProgramIdentifier),
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum MethodArgs {
    ApproveSignature(ProgramIdentifier, StateObject, Operation, Signature),
}

#[derive(Archive, Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum MethodReturns {
    ApproveSignature(bool),
}

// TODO: Remove later
impl Default for MethodReturns {
    fn default() -> Self { Self::ApproveSignature(true) }
}

pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::ApproveSignature(id, object, operation, signature) =>
            MethodReturns::ApproveSignature(approve_signature(id, object, operation, signature)),
    }
}

/// Hardcoded Pubkey
#[allow(dead_code)]
const PUB_KEY: [u8; 32] = [
    21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181, 21, 0,
    31, 100, 7, 100, 189, 2, 100,
];

// TODO: approves everything
pub fn approve_signature(
    self_prog_id: ProgramIdentifier,
    object: StateObject,
    _op: Operation,
    _signature: Signature,
) -> bool {
    event_emit(
        self_prog_id,
        mozak_sdk::coretypes::Event::ReadContextVariable(
            mozak_sdk::coretypes::ContextVariable::SelfProgramIdentifier(self_prog_id),
        ),
    );
    event_emit(
        self_prog_id,
        mozak_sdk::coretypes::Event::ReadStateObject(object.clone()),
    );
    true
}
