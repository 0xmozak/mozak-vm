#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Address, ProgramIdentifier, Signature, StateObject};
use mozak_sdk::cpc::cross_program_call;

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

pub fn transfer(
    self_prog_id: ProgramIdentifier,
    token_object: StateObject,
    remitter_signature: Signature,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) {
    assert!(cross_program_call(
        self_prog_id,
        remitter_wallet,
        wallet::MethodsIdentifiers::ApproveSignature as u8,
        wallet::MethodArgs::ApproveSignature(
            token_object,
            wallet::Operation::TransferTo(remittee_wallet),
            remitter_signature
        ),
        true,
    ));
}
