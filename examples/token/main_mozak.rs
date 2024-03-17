#![no_main]
#![allow(unused_attributes)]
#![feature(restricted_std)]

mod core_logic;

use core_logic::{dispatch, MethodArgs, MethodReturns};
use mozak_sdk::call_receive;

pub fn main() {
    while let Some((_caller, argument, return_)) = call_receive::<MethodArgs, MethodReturns>() {
        assert!(dispatch(argument) == return_);
    }
}

// We define `main()` to be the program's entry point.
mozak_sdk::entry!(main);

/* 

    cd examples && cargo build --release --bin tokenbin; \
    cd .. && MOZAK_STARK_DEBUG=true \
    ./target/release/mozak-cli prove-and-verify -vvv \
    examples/target/riscv32im-mozak-mozakvm-elf/release/tokenbin \
    --system-tape examples/token_tfr.tape.json \
    --self-prog-id \
    MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538;

*/
