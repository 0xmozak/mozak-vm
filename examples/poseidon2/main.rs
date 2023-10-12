#![no_main]
#![no_std]

use core::assert_eq;

use guest::hash::poseidon2_hash;
use hex_literal::hex;

pub fn main() {
    let data = "Mozak-VM Rocks";
    let hash = poseidon2_hash(data.as_bytes());
    assert_eq!(
        hash.as_bytes()[..],
        hex!("465f52a0c7950fb3b50ea67d5dcb37b92fdfba11a6ed00339cbaed70cae2482c")[..]
    );
    guest::env::write(hash.as_bytes());
}

guest::entry!(main);
