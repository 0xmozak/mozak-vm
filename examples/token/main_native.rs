#![feature(restricted_std)]
mod core_logic;
use std::fs::File;

use mozak_sdk::coretypes::{ProgramIdentifier, StateObject};
use mozak_sdk::io::{
    from_tape_deserialized, from_tape_function_id, from_tape_rawbuf, get_tapes_native,
    to_tape_function_id, to_tape_rawbuf, to_tape_serialized,
};
use mozak_sdk::cpc::globaltrace_dump_to_disk;
use simple_logger::{set_up_color_terminal, SimpleLogger};
use token::transfer;

fn main() {
    SimpleLogger::new().init().unwrap();
    set_up_color_terminal();

    log::info!("Running token-native");

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
        data: &[],
    };

    let remitter_signature = [70u8, 20, 56, 33];

    transfer(
        token_program,
        token_object,
        &remitter_signature,
        remitter_wallet,
        remittee_wallet,
    );

    globaltrace_dump_to_disk("wallet_transfer_cpc".to_string());

    log::info!("Generated tapes and verified proof, all done!");
}
