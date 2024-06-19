#![no_main]
#![allow(unused_attributes)]
#![no_std]
use mozak_sdk::{trace, trace_scope};

#[macro_use]
extern crate alloc;

pub fn main() {
    let mut message_bytes = vec![];

    trace_scope!({
        trace!("Debugging variables inside mozakvm is simple with trace!");
        trace!("Simply write the debug code, and use trace in this scope.");
        trace!("The code in this scope will be ignored when trace feature is off.");
        message_bytes.push(1);
    });

    // `message_bytes` remains unaffected when trace feature is off
    #[cfg(not(feature = "trace"))]
    assert!(message_bytes.len() == 0);

    trace!("trace doesn't work outside trace_scope!!. Will result in warning");

    message_bytes.extend([
        104, 101, 108, 108, 111, 32, 102, 114, 111, 109, 32, 109, 111, 122, 97, 107, 118, 109, 33,
    ]);

    trace_scope!({
        trace!("Lets decode the bytes to see our message");
        let message = alloc::string::String::from_utf8(message_bytes.to_vec()).unwrap();
        trace!("Here is the message: {message}");
    });
}

mozak_sdk::entry!(main);
