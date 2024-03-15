#![feature(restricted_std)]
#![allow(unused_attributes)]
mod core_logic;

use mozak_sdk::common::types::{Poseidon2Hash, ProgramIdentifier, StateAddress, StateObject};
use mozak_sdk::native::dump_system_tape;
use token::{dispatch, MethodArgs, MethodReturns};
// use wallet::TokenObject;

fn main() {
    let token_program = ProgramIdentifier::new_from_rand_seed(1);
    let remitter_program = ProgramIdentifier::new_from_rand_seed(2);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(3);
    let pub_key = wallet::PublicKey::new_from_rand_seed(4);

    let token_object = wallet::TokenObject {
        pub_key,
        amount: 100.into(),
    };

    let bytes = rkyv::to_bytes::<_, 256>(&token_object).unwrap();

    let state_object = StateObject {
        address: StateAddress::new_from_rand_seed(4),
        constraint_owner: token_program,
        // TODO(bing): encode a change in different economic owner in this `TokenObject`
        data: bytes.to_vec(),
    };

    mozak_sdk::call_send(
        token_program,
        MethodArgs::Transfer(
            state_object,
            remitter_program,
            remittee_program,
        ),
        dispatch,
    );
    // call_send(
    //     ProgramIdentifier::default(),
    //     token_program,
    //     MethodArgs::Transfer(
    //         token_program,
    //         state_object,
    //         remitter_wallet,
    //         remittee_wallet,
    //     ),
    //     dispatch,
    //     || -> MethodReturns {
    //         MethodReturns::Transfer // TODO read from
    //                                 // private tape
    //     },
    // );

    mozak_sdk::native::dump_system_tape("token_tfr", true);

    // println!("------>   Generated tapes!");
}
