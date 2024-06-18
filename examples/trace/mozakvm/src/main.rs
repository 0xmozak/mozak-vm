#![no_main]
#![allow(unused_attributes)]
#![no_std]

use mozak_sdk::core::ecall::trace;
#[macro_use]
extern crate alloc;

pub fn main() {
    trace("Debugging variables inside mozakvm is simple with trace!");
    let message_bytes = [
        104, 101, 108, 108, 111, 32, 102, 114, 111, 109, 32, 109, 111, 122, 97, 107, 118, 109, 33,
    ];
    trace(&format!(
        "Here is the message: {}",
        alloc::string::String::from_utf8(message_bytes.to_vec()).unwrap()
    ));
}

mozak_sdk::entry!(main);
