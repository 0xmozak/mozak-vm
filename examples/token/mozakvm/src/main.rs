#![no_main]
#![allow(unused_attributes)]
#![feature(restricted_std)]

use mozak_sdk::core::ecall::trace;
use mozak_sdk::call_receive;
use token_core_logic::{dispatch, MethodArgs, MethodReturns};

fn foo() {
    trace("foo");
}

pub fn main() {
    trace("Start of main");
    // use mozak_sdk::core::ecall::halt;
    // halt(0);
    // return;
    // let mut x = 0;
    trace("Start of main 2");
    foo();
    while let Some((_caller, argument, return_)) = call_receive::<MethodArgs, MethodReturns>() {
        trace("Loop!");
        // halt(0);
        // assert!(dispatch(argument) == return_);
        dispatch(argument);
        // x += 1;
        // if x > 0 {
        //     break;
        // }
    }
    trace("End of main");
}

// We define `main()` to be the program's entry point.
mozak_sdk::entry!(main);
