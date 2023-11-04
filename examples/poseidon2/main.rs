#![no_main]
#![no_std]

use core::assert_eq;

use guest::hash::poseidon2_hash;
use hex_literal::hex;

pub fn main() {
    let data = "Mozak-VM Rocks!!";
    let hash = poseidon2_hash(data.as_bytes());
    assert_eq!(
        hash.as_bytes()[..],
        hex!("5c2699dfd609d4566ee6656d2edb8298bacaccde758ec4f3005ff59a83347cd7")[..]
    );
    guest::env::write(hash.as_bytes());
}

guest::entry!(main);
