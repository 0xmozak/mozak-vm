#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]
#![cfg_attr(target_os = "mozakvm", no_main)]

use mozak_sdk::common::merkelize::merkleize;
use mozak_sdk::common::types::Poseidon2Hash;

pub fn main() {
    // test vectors from the corresponding merkelize test from sdk
    let mut addr = vec![0x010, 0x011, 0x011, 0x111];
    let mut hashes = vec![
        Poseidon2Hash([1u8; 32]),
        Poseidon2Hash([2u8; 32]),
        Poseidon2Hash([3u8; 32]),
        Poseidon2Hash([4u8; 32]),
    ];
    assert_eq!(merkleize(&mut addr, &mut hashes).inner(), [
        232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192, 132, 26, 242, 155, 95, 48, 48,
        8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53
    ]);
}

mozak_sdk::entry!(main);
