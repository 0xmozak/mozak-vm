#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]
#![cfg_attr(target_os = "mozakvm", no_main)]

pub fn main() {
    let data = "Mozak-VM Rocks!!";
    let hash = mozak_sdk::sys::poseidon2_hash_no_pad(data.as_bytes());
    core::assert_eq!(
        hash[..],
        hex_literal::hex!("5c2699dfd609d4566ee6656d2edb8298bacaccde758ec4f3005ff59a83347cd7")[..]
    );
    guest::env::write(&hash.to_le_bytes());
    let padded_hash = mozak_sdk::sys::poseidon2_hash_with_pad(data.as_bytes());
    core::assert_eq!(
        padded_hash[..],
        hex_literal::hex!("4f6b4db66bf76d93031979a3cdc27eb771d3fd94b52ba551b39f97ceb08c7d5c")[..]
    );
    guest::env::write(&padded_hash.to_le_bytes());
}

guest::entry!(main);
