#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Address, Poseidon2HashType, ProgramIdentifier, StateObject};
use mozak_sdk::cpc::cross_program_call;
use rkyv::{Archive, Deserialize, Serialize};

#[repr(u8)]
pub enum Methods {
    Mint,
    Burn,
    Transfer,
    GetAmount,
    Split,
}

// TODO: how do we verify owner?
pub fn mint(address: Address, amount: u64) {
    // TODO
}

pub fn burn(object: StateObject) {
    // TODO
}

pub fn split(original_object: StateObject, new_object_location: Address, new_object_amount: u64) {
    // TODO
}

pub fn transfer(object: StateObject, new_owner: ProgramIdentifier) {
    // TODO
}
