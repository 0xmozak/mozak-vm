#![no_main]
#![allow(unused_attributes)]
#![feature(restricted_std)]

mod core_logic;

use core_logic::{dispatch, MethodArgs, MethodReturns};
use mozak_sdk::call_receive;

pub fn main() {
    
}

// We define `main()` to be the program's entry point.
mozak_sdk::entry!(main);
