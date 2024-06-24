#![no_main]
#![allow(unused_attributes)]
#![no_std]

extern crate alloc;

use alloc::string::String;
use core::hint::black_box;

use mozak_sdk::debug_scope;
#[cfg(feature = "trace")]
use mozak_sdk::trace;
use rkyv::rancor::Panic;
use rkyv::util::AlignedVec;

use crate::alloc::string::ToString;

#[derive(Default, Clone, rkyv::Archive, Debug, rkyv::Serialize, rkyv::Deserialize)]
#[archive(check_bytes)]
pub struct Account {
    id: u32,
    owner: String,
    balance: u32,
}

// Usually we don't serialize inside vm, but for this example,
// we make an exception.
fn make_bytes() -> AlignedVec {
    let data = Account {
        id: 0x1234,
        owner: "Alice".to_string(),
        balance: 10000,
    };
    rkyv::to_bytes::<_, 256, Panic>(&data).unwrap()
}

pub fn main() {
    debug_scope!({
        trace!("Debugging variables inside mozakvm is simple with trace!");
        trace!("Simply write the debug code, and use trace in this scope.");
        trace!("The code in this scope will be ignored when trace feature is off.");
    });

    // imagine these bytes as coming from some raw tape.
    // we would like to deserialize
    #[allow(unused_variables)]
    let serialized_account_bytes = black_box(make_bytes());

    debug_scope!({
        trace!("Lets deserialize the bytes");
        let message: Account =
            rkyv::from_bytes::<Account, Panic>(&serialized_account_bytes).unwrap();
        trace!("Here is the deserialized struct: {message:?}");
    });
}

mozak_sdk::entry!(main);
