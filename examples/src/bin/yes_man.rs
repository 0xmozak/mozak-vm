#![no_main]
#![no_std]

use core::assert;

use examples::TMP;

fn yes_man() -> bool { true }

pub fn main() {
    assert!(TMP.is_empty());

    let valid = yes_man();
    assert!(valid);
    guest::env::write(&(valid as u32).to_le_bytes());
}

guest::entry!(main);
