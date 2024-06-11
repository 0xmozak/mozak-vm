#![no_main]
#![allow(unused_attributes)]
#![feature(restricted_std)]

use mozak_sdk::call_receive;
use wallet_core_logic::{dispatch, MethodArgs, MethodReturns};

pub fn main() {
    while let Some((_caller, argument, return_)) = call_receive::<MethodArgs, MethodReturns>() {
        assert!(dispatch(argument) == return_);
    }
}

// We define `main()` to be the program's entry point.
mozak_sdk::entry!(main);
