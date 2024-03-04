#![feature(restricted_std)]
extern crate alloc;

use mozak_sdk::coretypes::{Event, ProgramIdentifier, StateObject};
use mozak_sdk::sys::{call_send, event_emit};
use rkyv::{Archive, Deserialize, Serialize};
use wallet::TokenObject;

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    // Mint,
    // Burn,
    Transfer(
        ProgramIdentifier,
        StateObject,
        ProgramIdentifier,
        ProgramIdentifier,
    ),
    // GetAmount,
    // Split,
}

#[derive(Archive, Default, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodReturns {
    // TODO: Remove later
    #[default]
    Transfer,
}

#[allow(dead_code)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::Transfer(id, object, remitter, remittee) => {
            transfer(id, object, remitter, remittee);
            MethodReturns::Transfer
        }
    }
}

fn state_object_data_to_token_object(value: StateObject) -> TokenObject {
    let archived = unsafe { rkyv::archived_root::<TokenObject>(&value.data[..]) };
    let token_object: TokenObject = archived.deserialize(&mut rkyv::Infallible).unwrap();
    token_object
}

#[allow(dead_code)]
pub fn transfer(
    self_prog_id: ProgramIdentifier,
    state_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) {
    event_emit(self_prog_id, Event::ReadStateObject(state_object.clone()));
    let token_object: TokenObject = state_object_data_to_token_object(state_object.clone());
    assert_eq!(
        call_send(
            self_prog_id,
            remitter_wallet,
            wallet::MethodArgs::ApproveSignature(
                remitter_wallet,
                token_object.pub_key.clone(),
                wallet::BlackBox::new(remitter_wallet, remittee_wallet, token_object),
            ),
            wallet::dispatch,
            || -> wallet::MethodReturns {
                wallet::MethodReturns::ApproveSignature(()) // TODO read from
                                                            // private tape
            }
        ),
        wallet::MethodReturns::ApproveSignature(()),
        "wallet approval not found"
    );

    event_emit(self_prog_id, Event::UpdatedStateObject(state_object));
}
