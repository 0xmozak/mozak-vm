#![feature(restricted_std)]
#![allow(unused_attributes)]
mod core_logic;
use mozak_sdk::common::types::{ProgramIdentifier, StateAddress, StateObject};
use rkyv::rancor::Panic;
use token::{dispatch, MethodArgs};

fn main() {
    let token_program = ProgramIdentifier::new_from_rand_seed(1);

    // We assume both wallet are the same program for now
    let remitter_program = ProgramIdentifier::new_from_rand_seed(2);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(3);
    let remitter_private_key = wallet::PrivateKey::new_from_rand_seed(4);
    let remitter_public_key = wallet::PublicKey(mozak_sdk::native::helpers::poseidon2_hash_no_pad(
        &remitter_private_key.0,
    ));

    let remittee_private_key = wallet::PrivateKey::new_from_rand_seed(5);
    let remittee_public_key = wallet::PublicKey(mozak_sdk::native::helpers::poseidon2_hash_no_pad(
        &remittee_private_key.0,
    ));

    let token_object = wallet::TokenObject {
        pub_key: remitter_public_key,
        amount: 100.into(),
    };

    let bytes = rkyv::to_bytes::<_, 256, Panic>(&token_object).unwrap();

    let state_object = StateObject {
        address: StateAddress::new_from_rand_seed(4),
        constraint_owner: token_program,
        data: bytes.to_vec(),
    };

    mozak_sdk::call_send(
        token_program,
        MethodArgs::Transfer(
            state_object,
            remitter_program,
            remittee_program,
            remittee_public_key,
        ),
        dispatch,
    );

    mozak_sdk::native::dump_proving_files("token");
}
