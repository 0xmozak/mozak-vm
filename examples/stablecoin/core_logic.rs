extern crate alloc;

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

pub fn split_obj(
) -> () {
   
}
