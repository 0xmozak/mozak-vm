#![feature(restricted_std)]
mod core_logic;

use mozak_sdk::coretypes::{ProgramIdentifier, StateObject};
#[cfg(not(target_os = "zkvm"))]
use mozak_sdk::sys::dump_tapes;
use mozak_sdk::sys::{call_send, event_emit};
use token::{dispatch, transfer, MethodArgs, MethodReturns};

fn main() {
    println!("------>   Running token-native");

    let token_program = ProgramIdentifier {
        program_rom_hash: [11, 113, 20, 251].into(),
        memory_init_hash: [2, 31, 3, 62].into(),
        entry_point: 0,
    };

    let remitter_wallet = ProgramIdentifier {
        program_rom_hash: [21, 90, 121, 87].into(),
        memory_init_hash: [31, 35, 20, 189].into(),
        entry_point: 0,
    };

    let remittee_wallet = ProgramIdentifier {
        program_rom_hash: [0, 2, 121, 187].into(),
        memory_init_hash: [180, 19, 19, 56].into(),
        entry_point: 0,
    };

    let token_object = StateObject {
        address: [4, 0, 0, 0, 0, 0, 0, 0].into(),
        constraint_owner: token_program,
        data: vec![],
    };

    let remitter_signature = vec![70u8, 20, 56, 33].into();

    call_send(
        ProgramIdentifier::default(),
        token_program,
        MethodArgs::Transfer(
            token_program,
            token_object,
            remitter_signature,
            remitter_wallet,
            remittee_wallet,
        ),
        dispatch,
        || -> MethodReturns {
            MethodReturns::Transfer // TODO read from
                                    // private tape
        },
    );

    // transfer(
    //     token_program,
    //     token_object,
    //     remitter_signature,
    //     remitter_wallet,
    //     remittee_wallet,
    // );

    dump_tapes("wallet_tfr".to_string());

    println!("------>   Generated tapes!");
}
