#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

use core::assert_eq;

use hex_literal::hex;
use mozak_sdk::sys::poseidon2_hash;

pub fn main() {
    let data = "Mozak-VM Rocks!!";
    let hash = poseidon2_hash(data.as_bytes());
    assert_eq!(
        hash.0[..],
        hex!("5c2699dfd609d4566ee6656d2edb8298bacaccde758ec4f3005ff59a83347cd7")[..]
    );
    guest::env::write(&hash.0);
}

guest::entry!(main);
