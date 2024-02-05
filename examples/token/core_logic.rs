#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Event, ProgramIdentifier, Signature, StateObject};
use mozak_sdk::sys::{event_emit, mailbox_send};

#[repr(u8)]
pub enum Methods {
    Mint,
    Burn,
    Transfer,
    GetAmount,
    Split,
}

pub fn transfer(
    self_prog_id: ProgramIdentifier, // ContextVariables Table
    token_object: StateObject,       //
    remitter_signature: Signature,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) {
    assert!(mailbox_send(
        self_prog_id,
        remitter_wallet,
        wallet::MethodsIdentifiers::ApproveSignature as u8,
        wallet::MethodArgs::ApproveSignature(
            token_object.clone(),
            wallet::Operation::TransferTo(remittee_wallet),
            remitter_signature
        ),
        true,
    ));
    event_emit(Event::UpdatedStateObject(token_object));
}
