#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Event, ProgramIdentifier, Signature, StateObject};
use mozak_sdk::sys::{call_send, event_emit};

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
    assert_eq!(
        call_send(
            self_prog_id,
            remitter_wallet,
            wallet::MethodArgs::ApproveSignature(
                remitter_wallet,
                token_object.clone(),
                wallet::Operation::TransferTo(remittee_wallet),
                remitter_signature.clone()
            ),
            wallet::dispatch,
            || -> wallet::MethodReturns {
                wallet::MethodReturns::ApproveSignature(true) // TODO read from
                                                              // private tape
            }
        ),
        wallet::MethodReturns::ApproveSignature(true),
        "wallet approval not found"
    );
    event_emit(self_prog_id, Event::UpdatedStateObject(token_object));
}
