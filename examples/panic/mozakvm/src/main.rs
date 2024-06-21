#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

pub fn main() {
    panic!("Mozak VM panics 😱");
}

mozak_sdk::entry!(main);
