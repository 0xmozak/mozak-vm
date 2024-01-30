#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Address, Poseidon2HashType, ProgramIdentifier, StateObject};
use mozak_sdk::cpc::cross_program_call;
use rkyv::{Archive, Deserialize, Serialize};

#[repr(u8)]
pub enum Methods {
    ApproveSignature,
}

const PUB_KEY: [u8; 32] = [
    21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181, 21, 0,
    31, 100, 7, 100, 189, 2, 100,
];

// TODO: approves everything
pub fn approve_signature(object: StateObject, operation: &str, signature: &[u8]) -> bool { true }
