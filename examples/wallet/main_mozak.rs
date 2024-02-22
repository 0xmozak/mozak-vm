#![no_main]
#![feature(restricted_std)]

mod core_logic;

use mozak_sdk::io::{get_tapes, Extractor};

pub fn main() {

    let Some(message_from_token_program) = call_receive(){
        let CPCMessage{
            token_program,
            wallet_program,
            approve_call_args,
            approval,
        } = message_from_token_program.0;

        let MethodArgs::ApproveTransfer(token_object, remitter_wallet, remittee_wallet) = approve_call_args;
        return approval == approve_transfer(token_object, remitter_wallet, remittee_wallet);
    }
}

pub fn approve_transfer(
    token_object: StateObject, 
    remitter_wallet: ProgramIdentifier, 
    remittee_wallet: ProgramIdentifier) -> bool {

    let (mut public_tape, mut _private_tape) = get_tapes();

    let private_key = PrivateKey::from(_private_tape);
    let token_object_data = token_object.data.into();
    let public_key = token_object_data.public_key;
    assert_eq!(remitter_wallet, token_object_data.wallet);
    return private_key == poseidon2_hash(public_key);
}

    core_logic.approve_transfer(token_object, remitter_wallet, remittee_wallet);
}

// We define `main()` to be the program's entry point.
guest::entry!(main);
