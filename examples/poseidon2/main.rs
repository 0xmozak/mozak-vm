#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

pub fn main() {
    #[cfg(not(target_os = "mozakvm"))]
    {
        let data = "Mozak-VM Rocks!!";
        let hash = mozak_sdk::mozakvm::helpers::poseidon2_hash(data.as_bytes());
        core::assert_eq!(
            hash[..],
            hex_literal::hex!("5c2699dfd609d4566ee6656d2edb8298bacaccde758ec4f3005ff59a83347cd7")[..]
        );
        guest::env::write(&hash);
    }
}

mozak_sdk::entry!(main);
