#![feature(restricted_std)]
#![allow(unused_attributes)]
mod core_logic;

use mozak_sdk::coretypes::{Poseidon2HashType, ProgramIdentifier, StateObject};
use mozak_sdk::sys::call_send;
#[cfg(not(target_os = "mozakvm"))]
use mozak_sdk::sys::dump_tapes;
use token::{dispatch, MethodArgs, MethodReturns};
use wallet::TokenObject;

fn main() {
    println!("------>   Running token-native");

    let token_program = ProgramIdentifier::new(
        Poseidon2HashType::from([11, 113, 20, 251].repeat(8)),
        Poseidon2HashType::from([2, 31, 3, 62].repeat(8)),
        0,
    );
    let remitter_wallet = ProgramIdentifier::new(
        Poseidon2HashType::from([21, 90, 121, 87].repeat(8)),
        Poseidon2HashType::from([31, 35, 20, 189].repeat(8)),
        0,
    );

    let remittee_wallet = ProgramIdentifier::new(
        Poseidon2HashType::from([0, 2, 121, 187].repeat(8)),
        Poseidon2HashType::from([180, 19, 19, 56].repeat(8)),
        0,
    );

    let pub_key: wallet::PublicKey = [
        21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181,
        21, 0, 31, 100, 7, 100, 189, 2, 100,
    ]
    .into();

    let token_object = TokenObject {
        wallet_prog_id: remitter_wallet,
        pub_key,
        amount: 100.into(),
    };

    let bytes = rkyv::to_bytes::<_, 256>(&token_object).unwrap();

    let state_object = StateObject {
        address: [4, 0, 0, 0, 0, 0, 0, 0].into(),
        constraint_owner: token_program,
        // TODO(bing): encode a change in different economic owner in this `TokenObject`
        data: bytes.to_vec(),
    };

    call_send(
        ProgramIdentifier::default(),
        token_program,
        MethodArgs::Transfer(
            token_program,
            state_object,
            remitter_wallet,
            remittee_wallet,
        ),
        dispatch,
        || -> MethodReturns {
            MethodReturns::Transfer // TODO read from
                                    // private tape
        },
    );

    dump_tapes("token_tfr".to_string());

    println!("------>   Generated tapes!");
}
