#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Event, ProgramIdentifier, Signature, StateObject};
use mozak_sdk::sys::{event_emit, call_send};

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
    assert!(call_send(
        self_prog_id,
        remitter_wallet,
        wallet::MethodsIdentifiers::ApproveSignature as u8,
        wallet::MethodArgs::ApproveSignature(
            token_object.clone(),
            wallet::Operation::TransferTo(remittee_wallet),
            remitter_signature.clone()
        ),
        {
            #[cfg(not(target_os = "zkvm"))]
            {
                wallet::approve_signature(
                    token_object.clone(),
                    wallet::Operation::TransferTo(remittee_wallet),
                    remitter_signature,
                )
            }
            #[cfg(target_os = "zkvm")]
            {
                // TODO: private tape read
            }
        },
    ));
    event_emit(Event::UpdatedStateObject(token_object));
}
