#![feature(restricted_std)]
extern crate alloc;

use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    RawTapesTest,
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
        MethodArgs::RawTapesTest => {
            raw_tapes_test();
            MethodReturns::Noop
        }
    }
}

/// Checks if each element of input tape is one off from the other
pub fn raw_tapes_test() {
    #[cfg(target_os = "mozakvm")]
    {
        assert!(mozak_sdk::input_tape_len(&mozak_sdk::InputTapeType::PublicTape) == 32);
        assert!(mozak_sdk::input_tape_len(&mozak_sdk::InputTapeType::PrivateTape) == 32);

        let mut buf1 = [0u8; 32];
        let mut buf2 = [0u8; 32];

        let _ = mozak_sdk::read(&mozak_sdk::InputTapeType::PublicTape, &mut buf1[..]);
        let _ = mozak_sdk::read(&mozak_sdk::InputTapeType::PrivateTape, &mut buf2[..]);

        buf1.iter()
            .zip(buf2.iter())
            .for_each(|(x, y)| assert!(x.wrapping_add(1) == *y));
    }
}
