// #![feature(restricted_std)]
extern crate alloc;
// use alloc::vec::Vec;
use mozak_sdk::coretypes::{ProgramIdentifier, Signature, StateObject};
use rkyv::{Archive, Deserialize, Serialize};

#[repr(u8)]
pub enum MethodsIdentifiers {
    ApproveSignature,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub enum Operation {
    TransferTo(ProgramIdentifier),
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub enum MethodArgs {
    ApproveSignature(StateObject, Operation, Signature),
}

/// Hardcoded Pubkey
#[allow(dead_code)]
const PUB_KEY: [u8; 32] = [
    21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181, 21, 0,
    31, 100, 7, 100, 189, 2, 100,
];

// TODO: approves everything
pub fn approve_signature(_object: StateObject, _op: Operation, _signature: Signature) -> bool {
    true
}
