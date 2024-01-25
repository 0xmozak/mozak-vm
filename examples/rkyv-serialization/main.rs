#![cfg_attr(target_os = "zkvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

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

use alloc::string::ToString;
use alloc::vec;

pub fn main() {
    let value = Test {
        int: 42,
        string: "Mozak Rocks!!".to_string(),
        option: Some(vec![1, 2, 3, 4]),
    };

    // Serializing is as easy as a single function call
    let bytes = rkyv::to_bytes::<_, 256>(&value).unwrap();

    // Or you can use the unsafe API for maximum performance
    let archived = unsafe { rkyv::archived_root::<Test>(&bytes[..]) };
    assert_eq!(archived, &value);

    // And you can always deserialize back to the original type
    let deserialized: Test = archived.deserialize(&mut rkyv::Infallible).unwrap();
    assert_eq!(deserialized, value);
    #[cfg(not(target_os = "zkvm"))]
    println!("Deserialized Value: {:?}", deserialized);
    let bytes = rkyv::to_bytes::<_, 256>(&deserialized).unwrap();
    guest::env::write(&bytes);
}

guest::entry!(main);
