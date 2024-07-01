#![allow(unused_attributes)]
use mozak_sdk::common::types::{ProgramIdentifier, StateAddress, StateObject};
use rkyv::rancor::Panic;
use token_core_logic::{dispatch, MethodArgs};
use token_elf_data::TOKEN_SELF_PROG_ID;
use wallet_elf_data::WALLET_SELF_PROG_ID;

fn main() {
    let token_program = ProgramIdentifier::from(TOKEN_SELF_PROG_ID.to_string());

    // We assume both wallet are the same program for now
    let remitter_program = ProgramIdentifier::from(WALLET_SELF_PROG_ID.to_string());
    let remittee_program = ProgramIdentifier::new_from_rand_seed(3);
    let remitter_private_key = wallet_core_logic::PrivateKey::new_from_rand_seed(4);
    let remitter_public_key = wallet_core_logic::PublicKey(
        mozak_sdk::native::poseidon::poseidon2_hash_no_pad(&remitter_private_key.0),
    );

    mozak_sdk::add_identity(remitter_program); // Manual override for `IdentityStack`
    let _ = mozak_sdk::write(
        &mozak_sdk::InputTapeType::PrivateTape,
        &remitter_private_key.0[..],
    );
    mozak_sdk::rm_identity(); // Manual override for `IdentityStack`

    let remittee_private_key = wallet_core_logic::PrivateKey::new_from_rand_seed(5);
    let remittee_public_key = wallet_core_logic::PublicKey(
        mozak_sdk::native::poseidon::poseidon2_hash_no_pad(&remittee_private_key.0),
    );

    let token_object = wallet_core_logic::TokenObject {
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
