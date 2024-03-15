#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]
#![cfg_attr(target_os = "mozakvm", no_main)]

use mozak_sdk::coretypes::Poseidon2HashType;
use mozak_sdk::utils::merkleize;

pub fn main() {
    let hashes_with_addr = vec![
        (0x010, Poseidon2HashType([1u8; 32])),
        (0x011, Poseidon2HashType([2u8; 32])),
        (0x011, Poseidon2HashType([3u8; 32])),
        (0x111, Poseidon2HashType([4u8; 32])),
    ];
    assert_eq!(merkleize(&hashes_with_addr).to_le_bytes(), [
        232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192, 132, 26, 242, 155, 95, 48, 48,
        8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53
    ]);
}

guest::entry!(main);
