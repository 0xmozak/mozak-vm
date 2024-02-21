#![feature(restricted_std)]
extern crate alloc;

// use alloc::vec::Vec;
use mozak_sdk::coretypes::{Event, ProgramIdentifier, Signature, StateObject};
use mozak_sdk::sys::{call_send, event_emit};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum MethodArgs {
    // Mint,
    // Burn,
    Transfer(
        ProgramIdentifier,
        StateObject,
        Signature,
        ProgramIdentifier,
        ProgramIdentifier,
    ),
    // GetAmount,
    // Split,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum MethodReturns {
    Transfer,
}

// TODO: Remove later
impl Default for MethodReturns {
    fn default() -> Self { Self::Transfer }
}

#[allow(dead_code)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::Transfer(id, object, signature, remitter, remittee) => {
            transfer(id, object, signature, remitter, remittee);
            MethodReturns::Transfer
        }
    }
}

#[allow(dead_code)]
pub fn transfer(
    self_prog_id: ProgramIdentifier, // ContextVariables Table
    token_object: StateObject,       //
    remitter_signature: Signature,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) {
    event_emit(self_prog_id, Event::ReadStateObject(token_object.clone()));
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
    event_emit(self_prog_id, Event::UpdatedStateObject(StateObject{
        data: vec![200],
        ..token_object
    }));
}
