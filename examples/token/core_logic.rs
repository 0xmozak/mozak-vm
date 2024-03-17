#![feature(restricted_std)]
extern crate alloc;

use mozak_sdk::common::types::{Event, EventType, StateAddress, ProgramIdentifier, StateObject};
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
        wallet::PublicKey
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
        MethodArgs::Transfer(object, remitter, remittee, remitee_pubkey) => {
            transfer(object, remitter, remittee, remitee_pubkey);
            MethodReturns::Transfer
        }
    }
}

#[allow(dead_code)]
pub fn transfer(
    state_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
    remitee_pubkey: wallet::PublicKey
) {
    let read_event = Event {
        object: state_object.clone(),
        type_: EventType::Read,
    };
    mozak_sdk::event_emit(read_event);

    let mut token_object: wallet::TokenObject = state_object.clone().into();

    // Ensure spendability
    assert!(
        mozak_sdk::call_send(
            remitter_wallet,
            wallet::MethodArgs::ApproveSignature(
                token_object.pub_key.clone(),
                wallet::BlackBox::new(remitter_wallet, remittee_wallet, token_object.clone()),
            ),
            wallet::dispatch,
        ) == wallet::MethodReturns::ApproveSignature(()),
    );

    token_object.pub_key = remitee_pubkey;

    let bytes = rkyv::to_bytes::<_, 256>(&token_object).unwrap();

    let state_object = StateObject {
        data: bytes.to_vec(),
        ..state_object
    };

    let write_event = Event {
        object: state_object,
        type_: EventType::Write,
    };
    mozak_sdk::event_emit(write_event);
}
