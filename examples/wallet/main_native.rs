#![allow(unused_attributes)]

mod core_logic;

use mozak_sdk::common::types::ProgramIdentifier;

use crate::core_logic::{dispatch, BlackBox, MethodArgs, PublicKey, TokenObject};

fn main() {
    let wallet_program: ProgramIdentifier =
        std::fs::read_to_string("self_prog_id.txt").unwrap().into();
    let remitter_program = ProgramIdentifier::new_from_rand_seed(20);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(21);
    let public_key = PublicKey::new_from_rand_seed(4);

    let token_object = TokenObject {
        pub_key: public_key.clone(),
        amount: 10.into(),
    };

    let black_box = BlackBox {
        remitter_program,
        remittee_program,
        token_object,
    };

    mozak_sdk::call_send(
        wallet_program,
        MethodArgs::ApproveSignature(public_key, black_box.clone()),
        dispatch,
    );

    mozak_sdk::native::dump_proving_files();
}
