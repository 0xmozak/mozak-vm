#![feature(restricted_std)]
extern crate alloc;

use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    RawTapesTest([u8; 32], [u8; 32]),
}

#[derive(Archive, Default, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodReturns {
    #[default]
    Noop,
}

#[allow(dead_code)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::RawTapesTest(tape_1, tape_2) => {
            raw_tapes_test(tape_1, tape_2);
            MethodReturns::Noop
        }
    }
}

/// Checks if each element of input tape is one off from the other
pub fn raw_tapes_test(tape_1: [u8; 32], tape_2: [u8; 32]) {
    #[cfg(not(target_os = "mozakvm"))]
    {
        let _ = mozak_sdk::write(&mozak_sdk::InputTapeType::PublicTape, &tape_1);
        let _ = mozak_sdk::write(&mozak_sdk::InputTapeType::PrivateTape, &tape_2);
    }
    #[cfg(target_os = "mozakvm")]
    {
        tape_1
            .iter()
            .zip(tape_2.iter())
            .for_each(|(x, y)| assert!(x.wrapping_add(1) == *y));
    }
}
