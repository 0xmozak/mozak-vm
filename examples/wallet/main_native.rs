#![feature(restricted_std)]
#![allow(unused_attributes)]

mod core_logic;
use mozak_prover_sdk::prog_id_bytes::{ProgIdBytes, ELF_DIR};
use mozak_sdk::common::types::ProgramIdentifier;

use crate::core_logic::{dispatch, BlackBox, MethodArgs, PublicKey, TokenObject};

fn main() {
    let wallet_program: ProgramIdentifier =
        ProgIdBytes::from_elf(&format!("{}{}", ELF_DIR, "walletbin"))
            .unwrap()
            .into();
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

    mozak_sdk::native::dump_proving_files("wallet");
}
