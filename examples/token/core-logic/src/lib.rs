#![feature(restricted_std)]
extern crate alloc;

use mozak_sdk::common::types::{Event, EventType, ProgramIdentifier, StateObject};
use rkyv::rancor::Panic;
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
        wallet_core_logic::PublicKey,
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
        MethodArgs::Transfer(object, remitter, remittee, remittee_pubkey) => {
            transfer(object, remitter, remittee, remittee_pubkey);
            MethodReturns::Transfer
        }
    }
}

#[allow(dead_code)]
pub fn transfer(
    state_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
    remitee_pubkey: wallet_core_logic::PublicKey,
) {
    let read_event = Event {
        object: state_object.clone(),
        type_: EventType::Read,
    };
    mozak_sdk::event_emit(read_event);

    let mut token_object = wallet_core_logic::TokenObject::from(state_object.clone());

    // Ensure spendability
    assert!(
        mozak_sdk::call_send(
            remitter_wallet,
            wallet_core_logic::MethodArgs::ApproveSignature(
                token_object.pub_key.clone(),
                wallet_core_logic::BlackBox::new(
                    remitter_wallet,
                    remittee_wallet,
                    token_object.clone()
                ),
            ),
            wallet_core_logic::dispatch,
        ) == wallet_core_logic::MethodReturns::ApproveSignature(()),
    );

    token_object.pub_key = remitee_pubkey;

    let bytes = rkyv::to_bytes::<_, 256, Panic>(&token_object).unwrap();

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
