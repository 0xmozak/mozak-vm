#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

use core::assert_eq;

use hex_literal::hex;
use sha2::{Digest, Sha256};

pub fn main() {
    let hash = Sha256::digest(b"Mozak!!!");
    assert_eq!(
        hash[..],
        hex!("e7a1df42bf66b73aaa02ca5728ff2b5e6871dfaa456546111325dc479f2cb5e1")[..]
    );
    mozak_sdk::core::env::write(hash.as_slice());
}

mozak_sdk::entry!(main);
