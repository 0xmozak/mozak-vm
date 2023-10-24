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
        hex!("5c6eba0042e2892dde5da1f8902b000a5e1084a01fead64172c45fd93205f0e5")[..]
    );
    guest::env::write(hash.as_bytes());
}

guest::entry!(main);
