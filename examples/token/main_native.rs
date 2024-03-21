#![feature(restricted_std)]
#![allow(unused_attributes)]

mod core_logic;
use mozak_sdk::common::types::{Poseidon2Hash, ProgramIdentifier, StateAddress, StateObject};
use rkyv::rancor::Panic;
use token::{dispatch, MethodArgs};

fn main() {
    let token_program = ProgramIdentifier::new_from_rand_seed(1);

    // We assume both wallet are the same program for now
    let remitter_program = ProgramIdentifier::new_from_rand_seed(2);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(3);
    let remitter_pub_key = wallet::PublicKey::new_from_rand_seed(4);
    let remittee_pub_key = wallet::PublicKey::new_from_rand_seed(5);

    let token_object = wallet::TokenObject {
        pub_key: remitter_pub_key,
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
            remittee_pub_key,
        ),
        dispatch,
    );

    let name = "token_tfr".to_string();
    mozak_sdk::native::dump_proving_files(name.clone(), token_program);
}
