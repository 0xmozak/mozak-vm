#![no_main]
#![no_std]

use core::assert_eq;

use guest::hash::poseidon_hash;
use hex_literal::hex;

pub fn main() {
    let data = "Mozak-VM";
    let hash = poseidon_hash(data.as_bytes());
    assert_eq!(
        hash.as_bytes()[..],
        hex!("6f43508b66e312f0ff05382d9d8dbca3c4eb4d12e24ddd54b062f71c1c08e7a4")[..]
    );
    guest::env::write(hash.as_bytes());
}

guest::entry!(main);
