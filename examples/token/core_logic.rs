#![feature(restricted_std)]
extern crate alloc;

use mozak_sdk::common::types::{Event, EventType, ProgramIdentifier, StateObject};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    // Mint,
    // Burn,
    Transfer(
        StateObject,
        ProgramIdentifier,
        ProgramIdentifier,
    ),
    // GetAmount,
    // Split,
}

#[derive(Archive, Default, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodReturns {
    // TODO: Remove later
    #[default]
    Transfer,
}

#[allow(dead_code)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::Transfer(object, remitter, remittee) => {
            transfer(object, remitter, remittee);
            MethodReturns::Transfer
        }
    }
}

#[allow(dead_code)]
pub fn transfer(
    state_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) {
    let read_event = Event {
        object: state_object.clone(),
        type_: EventType::Read,
    };
    mozak_sdk::event_emit(read_event);

    let token_object: wallet::TokenObject = state_object.clone().into();

    assert!(
        mozak_sdk::call_send(
            remitter_wallet,
            wallet::MethodArgs::ApproveSignature(
                token_object.pub_key.clone(),
                wallet::BlackBox::new(remitter_wallet, remittee_wallet, token_object),
            ),
            wallet::dispatch,
        ) == wallet::MethodReturns::ApproveSignature(()),
    );

    let write_event = Event {
        object: state_object,
        type_: EventType::Write,
    };
    mozak_sdk::event_emit(write_event);
}
