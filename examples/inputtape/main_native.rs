#![feature(restricted_std)]
#![allow(unused_attributes)]
mod core_logic;

use mozak_sdk::common::types::{Poseidon2Hash};

fn main() {
    let raw_tape_1 = Poseidon2Hash::new_from_rand_seed(1).inner();
    let raw_tape_2 = raw_tape_1.iter().map(|x| x.wrapping_add(1)).inner();
    
    mozak_sdk::native::dump_system_tape("inputtape", true);
}
