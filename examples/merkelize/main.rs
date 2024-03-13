#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]
#![cfg_attr(target_os = "mozakvm", no_main)]

use mozak_sdk::commit_event_tape::merklelize;
use mozak_sdk::coretypes::Poseidon2HashType;

pub fn main() {
    let hashes_with_addr = vec![
        (200, Poseidon2HashType([1u8; 32])),
        (100, Poseidon2HashType([2u8; 32])),
        (300, Poseidon2HashType([3u8; 32])),
    ];
    assert_eq!(merklelize(hashes_with_addr).to_le_bytes(), [
        148, 89, 143, 68, 91, 176, 146, 6, 215, 130, 191, 141, 189, 91, 42, 189, 195, 109, 98, 80,
        53, 18, 90, 71, 164, 178, 181, 93, 211, 99, 80, 86
    ]);
}

guest::entry!(main);
