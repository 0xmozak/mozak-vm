#![feature(restricted_std)]
mod core_logic;

use mozak_sdk::coretypes::{Address, Poseidon2HashType, ProgramIdentifier, StateObject};
use simple_logger::{set_up_color_terminal, SimpleLogger};

use crate::core_logic::{swap_tokens, MetadataObject};

fn main() {
    SimpleLogger::new().init().unwrap();
    set_up_color_terminal();

    log::info!("Running amm-native");

    let amm_program = ProgramIdentifier {
        program_rom_hash: [1, 113, 100, 251].into(),
        memory_init_hash: [231, 31, 37, 62].into(),
        entry_point: 0,
    };

    let usdc_program = ProgramIdentifier {
        program_rom_hash: [21, 33, 121, 51].into(),
        memory_init_hash: [31, 35, 221, 189].into(),
        entry_point: 0,
    };
    let usdt_program = ProgramIdentifier {
        program_rom_hash: [19, 139, 201, 77].into(),
        memory_init_hash: [2, 100, 20, 62].into(),
        entry_point: 0,
    };

    let metadata_object = MetadataObject {
        token_programs: [usdc_program, usdt_program],
        reserves: [10000, 10000],
    };

    let amount_in: u64 = 90;

    let user_wallet = ProgramIdentifier {
        program_rom_hash: [7, 9, 28, 82].into(),
        memory_init_hash: [183, 81, 138, 6].into(),
        entry_point: 0,
    };

    let available_state_addresses = [
        [3, 0, 0, 0, 0, 0, 0, 0].into(),
        [3, 0, 0, 0, 0, 0, 0, 1].into(),
    ];

    let objects_presented = vec![
        StateObject {
            address: [1, 0, 0, 0, 0, 0, 0, 0].into(),
            constraint_owner: usdc_program,
            data: &[],
        },
        StateObject {
            address: [1, 0, 0, 0, 0, 0, 0, 1].into(),
            constraint_owner: usdc_program,
            data: &[],
        },
    ];

    let objects_requested = vec![
        StateObject {
            address: [2, 0, 0, 0, 0, 0, 0, 0].into(),
            constraint_owner: usdt_program,
            data: &[],
        },
        StateObject {
            address: [2, 0, 0, 0, 0, 0, 0, 1].into(),
            constraint_owner: usdt_program,
            data: &[],
        },
    ];

    swap_tokens(
        metadata_object,
        amount_in,
        user_wallet,
        objects_presented,
        objects_requested,
        available_state_addresses,
        amm_program,
    );

    // let files = ["public_input.tape", "private_input.tape"];

    log::info!("Generated tapes and verified proof, all done!");
}
