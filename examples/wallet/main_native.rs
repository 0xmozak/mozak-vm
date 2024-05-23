#![allow(unused_attributes)]

mod core_logic;

use mozak_sdk::common::types::ProgramIdentifier;

use crate::core_logic::{dispatch, BlackBox, MethodArgs, PrivateKey, PublicKey, TokenObject};

fn main() {
    let wallet_program = ProgramIdentifier::new_from_rand_seed(1);
    let remitter_program = ProgramIdentifier::new_from_rand_seed(2);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(3);
    let private_key = PrivateKey::new_from_rand_seed(4);
    let public_key = PublicKey(mozak_sdk::native::helpers::poseidon2_hash_no_pad(
        &private_key.0,
    ));
    mozak_sdk::add_identity(remitter_program); // Manual override for `IdentityStack`
    let _ = mozak_sdk::write(&mozak_sdk::InputTapeType::PrivateTape, &private_key.0[..]);
    mozak_sdk::rm_identity(); // Manual override for `IdentityStack`

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
