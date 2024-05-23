#![allow(unused_attributes)]
mod core_logic;
use mozak_sdk::common::types::{ProgramIdentifier, StateAddress, StateObject};
use rkyv::rancor::Panic;
use token::{dispatch, MethodArgs};
fn main() {
    let token_program =
        ProgramIdentifier::new_from_rand_seed(crate::core_logic::TOKEN_PROGRAM_SEED);

    // We assume both wallet are the same program for now
    let remitter_program = ProgramIdentifier::new_from_rand_seed(wallet::REMITTER_WALLET_SEED);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(wallet::REMITTEE_WALLET_SEED);
    let remitter_private_key = wallet::PrivateKey::new_from_rand_seed(wallet::REMITTER_WALLET_SEED);
    let remitter_public_key = wallet::PublicKey(mozak_sdk::native::helpers::poseidon2_hash_no_pad(
        &remitter_private_key.0,
    ));

    let remittee_private_key = wallet::PrivateKey::new_from_rand_seed(wallet::REMITTEE_WALLET_SEED);
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

    mozak_sdk::native::dump_proving_files();
}
