#![no_main]
#![feature(restricted_std)]

mod core_logic;

use crate::core_logic::min_max;

pub fn main() {
    let result = min_max();
    guest::env::write(&result);
}

guest::entry!(main);
