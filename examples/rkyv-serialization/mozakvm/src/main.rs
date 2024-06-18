#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive(
    // This will generate a PartialEq impl between our unarchived and archived
    // types:
    compare(PartialEq),
)]
// Derives can be passed through to the generated type:
#[archive_attr(derive(Debug))]
pub struct Test {
    pub int: u8,
    pub string: String,
    pub option: Option<Vec<i32>>,
}

#[cfg(not(feature = "std"))]
use alloc::string::ToString;
use alloc::vec;

pub fn main() {
    let value = Test {
        int: 42,
        string: "Mozak Rocks!!".to_string(),
        option: Some(vec![1, 2, 3, 4]),
    };

    // Serializing is as easy as a single function call
    let bytes = rkyv::to_bytes::<_, 256, Panic>(&value).unwrap();

    // Or you can use the unsafe API for maximum performance
    let archived = unsafe { rkyv::access_unchecked::<Test>(&bytes[..]) };
    assert_eq!(archived, &value);

    // And you can always deserialize back to the original type
    let deserialized: Test = archived
        .deserialize(Strategy::<(), Panic>::wrap(&mut ()))
        .unwrap();
    assert_eq!(deserialized, value);
    #[cfg(all(not(target_os = "mozakvm"), feature = "std"))]
    println!("Deserialized Value: {:?}", deserialized);
    let bytes = rkyv::to_bytes::<_, 256, Panic>(&deserialized).unwrap();
    mozak_sdk::core::env::write(&bytes);
}

mozak_sdk::entry!(main);
