extern crate alloc;

use std::ops::Add;

// use alloc::vec::Vec;
use mozak_sdk::{
    coretypes::{Address, Poseidon2HashType, ProgramIdentifier, StateObject},
    cpc::cross_program_call,
};
use rkyv::{Archive, Deserialize, Serialize};

pub enum Methods {
    MINT,
    BURN,
    TRANSFER,
    SPLIT,
}

// TODO: how do we verify owner?
pub fn mint(
    address: Address,
    amount: u64
) {
    // TODO
}

pub fn burn(
    object: StateObject
) {
    // TODO
}

pub fn split(
    original_object: StateObject,
    new_object_location: Address,
    new_object_amount: u64
) {
    // TODO
}

pub fn transfer(
    object: StateObject,
    new_owner: ProgramIdentifier
) {
    // TODO
}
