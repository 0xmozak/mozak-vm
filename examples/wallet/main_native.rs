#![feature(restricted_std)]

mod core_logic;

use mozak_sdk::coretypes::ProgramIdentifier;
use mozak_sdk::sys::{call_send, dump_tapes};

use crate::core_logic::{dispatch, BlackBox, MethodArgs, MethodReturns, PublicKey, TokenObject};

fn main() {
    println!("------>   Running wallet-native");

    let wallet_program = ProgramIdentifier {
        program_rom_hash: [11, 113, 20, 251].into(),
        memory_init_hash: [2, 31, 3, 62].into(),
        entry_point: 0,
    };

    let remittee_wallet = ProgramIdentifier {
        program_rom_hash: [0, 2, 121, 187].into(),
        memory_init_hash: [180, 19, 19, 56].into(),
        entry_point: 0,
    };

    let remitter_wallet = ProgramIdentifier {
        program_rom_hash: [21, 90, 121, 87].into(),
        memory_init_hash: [31, 35, 20, 189].into(),
        entry_point: 0,
    };

    let pub_key: PublicKey = [
        21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181,
        21, 0, 31, 100, 7, 100, 189, 2, 100,
    ]
    .into();

    let token_object = TokenObject {
        wallet_prog_id: remittee_wallet,
        pub_key: pub_key.clone(),
        amount: 10.into(),
    };

    let black_box = BlackBox {
        remitter_wallet,
        remittee_wallet,
        token_object,
    };

    call_send(
        ProgramIdentifier::default(),
        wallet_program,
        MethodArgs::ApproveSignature(wallet_program, pub_key.clone(), black_box.clone()),
        dispatch,
        || -> MethodReturns { MethodReturns::ApproveSignature(()) },
    );
    core_logic::approve_signature(remittee_wallet, pub_key, black_box);

    dump_tapes("wallet_approve".to_string());
    println!("------>   Generated tapes!");
}
