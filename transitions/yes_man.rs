#![no_main]
#![no_std]

use core::assert;

fn yes_man() -> bool { true }

pub fn main() {
    let valid = yes_man();
    assert!(valid);
    guest::env::write(&(valid as u32).to_le_bytes());
}

guest::entry!(main);
