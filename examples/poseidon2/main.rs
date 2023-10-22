#![no_main]
#![no_std]

use core::assert_eq;

use guest::hash::poseidon2_hash;
use hex_literal::hex;

pub fn main() {
    let data = "Mozak-VM";
    let hash = poseidon2_hash(data.as_bytes());
    assert_eq!(
        hash.as_bytes()[..],
        hex!("aad35ba40f7c24e9f45292e8d3d0b26f492b0921cf534b2301d8085501f1176d")[..]
    );
    guest::env::write(hash.as_bytes());
}

guest::entry!(main);
