#![no_main]
#![no_std]

use mozak_transitions::deserialize_input;

pub fn main() {
    let valid = !deserialize_input().is_empty();

    assert!(valid);
    guest::env::write(&(valid as u32).to_le_bytes());
}

guest::entry!(main);
